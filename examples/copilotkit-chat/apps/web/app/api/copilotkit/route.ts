import { NextRequest } from "next/server";
import { execFile } from "node:child_process";
import { randomUUID } from "node:crypto";
import { resolve } from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

type CopilotRunMessage = {
  role?: string;
  content?: unknown;
};

type CopilotRequestPayload = {
  method?: string;
  params?: {
    agentId?: string;
  };
  body?: {
    threadId?: string;
    runId?: string;
    messages?: CopilotRunMessage[];
  };
};

const buildSseResponse = (events: unknown[]): Response => {
  const payload = events.map((event) => `data: ${JSON.stringify(event)}\n\n`).join("");

  return new Response(payload, {
    status: 200,
    headers: {
      "content-type": "text/event-stream; charset=utf-8",
      "cache-control": "no-cache, no-transform",
      connection: "keep-alive",
    },
  });
};

const buildEmptySseResponse = (): Response => {
  return new Response("", {
    status: 200,
    headers: {
      "content-type": "text/event-stream; charset=utf-8",
      "cache-control": "no-cache, no-transform",
      connection: "keep-alive",
    },
  });
};

const buildInfoResponse = (): Response => {
  return Response.json({
    version: "1.52.1",
    agents: {
      default: {
        name: "default",
        description: "",
        className: "BuiltInAgent",
      },
    },
    audioFileTranscriptionEnabled: false,
  });
};

const isRevenueByRegionIntent = (text: string): boolean =>
  /revenue/i.test(text) && /region/i.test(text);

const getLatestUserText = (messages: CopilotRunMessage[] | undefined): string => {
  if (!messages || messages.length === 0) {
    return "";
  }

  for (let idx = messages.length - 1; idx >= 0; idx -= 1) {
    const message = messages[idx];
    if (message.role !== "user") {
      continue;
    }

    if (typeof message.content === "string") {
      return message.content;
    }
  }

  return "";
};

const extractRegionRevenueRows = (
  rows: Array<Record<string, unknown>>,
): Array<{ region: string; revenue: number }> => {
  return rows
    .map((row) => {
      const regionEntry = Object.entries(row).find(([key]) => /region/i.test(key));
      const revenueEntry = Object.entries(row).find(([key]) => /revenue/i.test(key));

      const region = typeof regionEntry?.[1] === "string" ? regionEntry[1] : String(regionEntry?.[1] ?? "");
      const revenueValue =
        typeof revenueEntry?.[1] === "number"
          ? revenueEntry[1]
          : Number(revenueEntry?.[1] ?? 0);

      return {
        region,
        revenue: Number.isFinite(revenueValue) ? revenueValue : 0,
      };
    })
    .filter((item) => item.region.length > 0);
};

const runQuarryRevenueByRegion = async (tenantId: string): Promise<string> => {
  const repoRoot = resolve(process.cwd(), "../../../..");
  const queryInputPath = resolve(repoRoot, "models/example/query_by_region.json");

  try {
    const { stdout } = await execFileAsync(
      "cargo",
      [
        "run",
        "-q",
        "-p",
        "quarry-cli",
        "--",
        "query",
        "--model",
        "models/example/model.yml",
        "--catalog",
        "local",
        "--tenant",
        tenantId,
        "--local-data-dir",
        "models/example/data",
        "--input",
        queryInputPath,
        "--format",
        "json",
      ],
      {
        cwd: repoRoot,
        timeout: 120000,
        maxBuffer: 10 * 1024 * 1024,
      },
    );

    const parsed = JSON.parse(stdout) as {
      status?: string;
      data?: { rows?: Array<Record<string, unknown>> };
      error?: { message?: string };
    };

    if (parsed.status !== "ok") {
      return `Quarry returned an error: ${parsed.error?.message ?? "unknown error"}`;
    }

    const rows = extractRegionRevenueRows(parsed.data?.rows ?? []);
    if (rows.length === 0) {
      return `No revenue rows were returned for tenant ${tenantId}.`;
    }

    const total = rows.reduce((sum, row) => sum + row.revenue, 0);
    const lines = rows.map((row) => `- ${row.region}: ${row.revenue.toFixed(1)}`).join("\n");

    return `Revenue by region for ${tenantId}:\n${lines}\n- Total: ${total.toFixed(1)}`;
  } catch (error) {
    const details = error instanceof Error ? error.message : String(error);
    return `Failed to run Quarry query: ${details}`;
  }
};

const buildAssistantRunResponse = (
  threadId: string,
  runId: string,
  assistantText: string,
): Response => {
  const messageId = randomUUID();

  return buildSseResponse([
    { type: "RUN_STARTED", threadId, runId },
    { type: "TEXT_MESSAGE_START", messageId, role: "assistant" },
    { type: "TEXT_MESSAGE_CONTENT", messageId, delta: assistantText },
    { type: "TEXT_MESSAGE_END", messageId },
    { type: "RUN_FINISHED", threadId, runId },
  ]);
};

export const POST = async (req: NextRequest) => {
  const parsedRequest = await req
    .json()
    .catch(() => null as CopilotRequestPayload | null);

  if (!parsedRequest?.method) {
    return Response.json(
      {
        error: "INVALID_REQUEST",
        message: "Expected a valid CopilotKit method payload",
      },
      { status: 400 },
    );
  }

  if (parsedRequest.method === "info") {
    return buildInfoResponse();
  }

  if (parsedRequest.method === "agent/connect") {
    return buildEmptySseResponse();
  }

  if (parsedRequest.method === "agent/run") {
    const threadId = parsedRequest.body?.threadId || randomUUID();
    const runId = parsedRequest.body?.runId || randomUUID();
    const latestUserText = getLatestUserText(parsedRequest.body?.messages);

    if (isRevenueByRegionIntent(latestUserText)) {
      const responseText = await runQuarryRevenueByRegion("tenant_123");
      return buildAssistantRunResponse(threadId, runId, responseText);
    }

    return buildAssistantRunResponse(
      threadId,
      runId,
      'Try: "Run Quarry query for tenant_123 and summarize revenue by region."',
    );
  }

  return Response.json(
    {
      error: "UNSUPPORTED_METHOD",
      message: `Unsupported method: ${parsedRequest.method}`,
    },
    { status: 400 },
  );
};
