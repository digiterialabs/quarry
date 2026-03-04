import type { Metadata } from "next";

import { CopilotKit } from "@copilotkit/react-core";
import "./globals.css";
import "@copilotkit/react-ui/v2/styles.css";

export const metadata: Metadata = {
  title: "Quarry CopilotKit Demo",
  description: "CopilotKit chat UI wired to Quarry MCP tools",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  const ollamaBaseUrl = process.env.OLLAMA_BASE_URL || "http://127.0.0.1:11434/v1";
  const ollamaModel = process.env.OLLAMA_MODEL || "llama3.1:8b";

  return (
    <html lang="en">
      <body className={"antialiased"}>
        <div className="bg-sky-200 px-4 py-3 text-sm text-sky-950">
          <strong>Local model mode:</strong> this UI uses Ollama at <code>{ollamaBaseUrl}</code>{" "}
          with model <code>{ollamaModel}</code>.
        </div>
        <CopilotKit runtimeUrl="/api/copilotkit" showDevConsole={false}>
          {children}
        </CopilotKit>
      </body>
    </html>
  );
}
