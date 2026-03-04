import { createMcpExpressApp } from "@modelcontextprotocol/sdk/server/express.js";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import cors from "cors";
import { spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { z } from "zod";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(
  process.env.QUARRY_REPO_ROOT || path.join(__dirname, "../../../.."),
);

const defaultModelPath =
  process.env.QUARRY_MODEL_PATH || path.join(repoRoot, "models/example/model.yml");
const defaultLocalDataDir =
  process.env.QUARRY_LOCAL_DATA_DIR || path.join(repoRoot, "models/example/data");
const defaultContextDir =
  process.env.QUARRY_CONTEXT_DIR || path.join(repoRoot, ".quarry-copilotkit-context");
const defaultQueryFile =
  process.env.QUARRY_QUERY_FILE || path.join(repoRoot, "models/example/query_by_region.json");

function quarryBaseCommand() {
  const quarryBin = (process.env.QUARRY_BIN || "").trim();
  if (quarryBin) return [quarryBin];
  return ["cargo", "run", "-q", "-p", "quarry-cli", "--"];
}

function runQuarry(args) {
  const cmd = quarryBaseCommand();
  const [program, ...prefixArgs] = cmd;
  const proc = spawnSync(program, [...prefixArgs, ...args], {
    cwd: repoRoot,
    encoding: "utf-8",
  });

  if (proc.status !== 0) {
    const stderr = (proc.stderr || "").trim();
    const stdout = (proc.stdout || "").trim();
    throw new Error(stderr || stdout || `quarry command failed (exit ${proc.status})`);
  }

  const text = (proc.stdout || "").trim();
  if (!text) return { raw: "" };
  try {
    return JSON.parse(text);
  } catch {
    return { raw: text };
  }
}

async function maybeWriteTempJson(prefix, value) {
  if (!value) return null;
  const filePath = path.join(
    os.tmpdir(),
    `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}.json`,
  );
  await fs.writeFile(filePath, JSON.stringify(value, null, 2), "utf-8");
  return filePath;
}

function toToolResult(payload) {
  return {
    content: [{ type: "text", text: JSON.stringify(payload, null, 2) }],
    structuredContent: payload,
  };
}

function createServer() {
  const server = new McpServer({
    name: "Quarry MCP HTTP Server",
    version: "0.2.0",
  });

  server.registerTool(
    "quarry_validate",
    {
      title: "Validate Quarry model",
      description: "Validate semantic model YAML.",
      inputSchema: {
        model_path: z.string().default(defaultModelPath),
      },
    },
    async ({ model_path }) => {
      const payload = runQuarry(["validate", "--model", model_path]);
      return toToolResult(payload);
    },
  );

  server.registerTool(
    "quarry_query",
    {
      title: "Run Quarry query",
      description: "Execute semantic query for a tenant.",
      inputSchema: {
        model_path: z.string().default(defaultModelPath),
        catalog: z.enum(["local", "glue"]).default("local"),
        tenant_id: z.string().default("tenant_123"),
        local_data_dir: z.string().default(defaultLocalDataDir),
        query_file: z.string().optional(),
        query_json: z.any().optional(),
      },
    },
    async ({ model_path, catalog, tenant_id, local_data_dir, query_file, query_json }) => {
      let inputPath = query_file || defaultQueryFile;
      let tempPath = null;
      if (!query_file && query_json) {
        tempPath = await maybeWriteTempJson("quarry-copilot-query", query_json);
        inputPath = tempPath || inputPath;
      }

      try {
        const payload = runQuarry([
          "query",
          "--model",
          model_path,
          "--catalog",
          catalog,
          "--tenant",
          tenant_id,
          "--local-data-dir",
          local_data_dir,
          "--input",
          inputPath,
          "--format",
          "json",
        ]);
        return toToolResult(payload);
      } finally {
        if (tempPath) {
          await fs.rm(tempPath, { force: true }).catch(() => undefined);
        }
      }
    },
  );

  server.registerTool(
    "quarry_explain",
    {
      title: "Explain Quarry plan",
      description: "Resolve semantic query and return query plan envelope.",
      inputSchema: {
        model_path: z.string().default(defaultModelPath),
        catalog: z.enum(["local", "glue"]).default("local"),
        tenant_id: z.string().default("tenant_123"),
        local_data_dir: z.string().default(defaultLocalDataDir),
        query_file: z.string().optional(),
        query_json: z.any().optional(),
      },
    },
    async ({ model_path, catalog, tenant_id, local_data_dir, query_file, query_json }) => {
      let inputPath = query_file || defaultQueryFile;
      let tempPath = null;
      if (!query_file && query_json) {
        tempPath = await maybeWriteTempJson("quarry-copilot-explain", query_json);
        inputPath = tempPath || inputPath;
      }

      try {
        const payload = runQuarry([
          "explain",
          "--model",
          model_path,
          "--catalog",
          catalog,
          "--tenant",
          tenant_id,
          "--local-data-dir",
          local_data_dir,
          "--input",
          inputPath,
        ]);
        return toToolResult(payload);
      } finally {
        if (tempPath) {
          await fs.rm(tempPath, { force: true }).catch(() => undefined);
        }
      }
    },
  );

  server.registerTool(
    "quarry_collection_create",
    {
      title: "Create collection",
      description: "Create tenant-scoped context collection.",
      inputSchema: {
        tenant_id: z.string().default("tenant_123"),
        name: z.string().default("sales_docs"),
        description: z.string().optional(),
        context_dir: z.string().default(defaultContextDir),
      },
    },
    async ({ tenant_id, name, description, context_dir }) => {
      const args = [
        "collection",
        "create",
        "--tenant",
        tenant_id,
        "--name",
        name,
        "--context-dir",
        context_dir,
      ];
      if (description) args.push("--description", description);
      const payload = runQuarry(args);
      return toToolResult(payload);
    },
  );

  server.registerTool(
    "quarry_sync",
    {
      title: "Sync collection",
      description: "Sync local docs or URLs into collection.",
      inputSchema: {
        tenant_id: z.string().default("tenant_123"),
        collection: z.string().default("sales_docs"),
        connector: z.enum(["filesystem", "url_list"]).default("filesystem"),
        context_dir: z.string().default(defaultContextDir),
        config_file: z.string().optional(),
        config_json: z.any().optional(),
      },
    },
    async ({ tenant_id, collection, connector, context_dir, config_file, config_json }) => {
      let configPath = config_file;
      let tempPath = null;

      if (!configPath) {
        const config =
          config_json ||
          (connector === "filesystem"
            ? { paths: [path.join(repoRoot, "models/example/context")], recursive: true, extensions: ["txt", "md"] }
            : { urls: [] });
        tempPath = await maybeWriteTempJson("quarry-copilot-sync", config);
        configPath = tempPath;
      }

      if (!configPath) {
        throw new Error("Missing sync config. Provide config_file or config_json.");
      }

      try {
        const payload = runQuarry([
          "sync",
          "--tenant",
          tenant_id,
          "--collection",
          collection,
          "--connector",
          connector,
          "--config",
          configPath,
          "--context-dir",
          context_dir,
        ]);
        return toToolResult(payload);
      } finally {
        if (tempPath) {
          await fs.rm(tempPath, { force: true }).catch(() => undefined);
        }
      }
    },
  );

  server.registerTool(
    "quarry_search",
    {
      title: "Search context",
      description: "Search indexed chunks for a tenant collection.",
      inputSchema: {
        tenant_id: z.string().default("tenant_123"),
        collection: z.string().default("sales_docs"),
        query: z.string(),
        top_k: z.number().int().positive().default(5),
        hybrid: z.enum(["on", "off"]).default("off"),
        context_dir: z.string().default(defaultContextDir),
      },
    },
    async ({ tenant_id, collection, query, top_k, hybrid, context_dir }) => {
      const payload = runQuarry([
        "search",
        "--tenant",
        tenant_id,
        "--collection",
        collection,
        "--query",
        query,
        "--top-k",
        String(top_k),
        "--hybrid",
        hybrid,
        "--context-dir",
        context_dir,
      ]);
      return toToolResult(payload);
    },
  );

  return server;
}

function startServer(port = 3108) {
  const app = createMcpExpressApp({ host: "0.0.0.0" });
  app.use(cors());

  app.all("/mcp", async (req, res) => {
    const server = createServer();
    const transport = new StreamableHTTPServerTransport({
      sessionIdGenerator: undefined,
    });

    res.on("close", () => {
      transport.close().catch(() => undefined);
      server.close().catch(() => undefined);
    });

    try {
      await server.connect(transport);
      await transport.handleRequest(req, res, req.body);
    } catch (error) {
      console.error("MCP error:", error);
      if (!res.headersSent) {
        res.status(500).json({
          jsonrpc: "2.0",
          error: { code: -32603, message: "Internal server error" },
          id: null,
        });
      }
    }
  });

  const httpServer = app.listen(port, () => {
    console.log(`Quarry MCP HTTP server listening on http://localhost:${port}/mcp`);
  });

  const shutdown = () => {
    httpServer.close(() => process.exit(0));
  };
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}

const port = Number.parseInt(process.env.PORT || "3108", 10);
startServer(Number.isNaN(port) ? 3108 : port);
