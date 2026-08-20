import { ComputerServer } from "./server.js";

const server = new ComputerServer();

try {
  await server.listen();
  console.log(`computer service listening on http://${server.address.host}:${server.address.port}`);
} catch (error) {
  console.error("failed to start computer service", error);
  process.exitCode = 1;
}

const shutdown = async (signal: string) => {
  console.log(`received ${signal}, shutting down computer service`);
  await server.close();
  process.exit(0);
};

process.once("SIGINT", () => void shutdown("SIGINT"));
process.once("SIGTERM", () => void shutdown("SIGTERM"));
