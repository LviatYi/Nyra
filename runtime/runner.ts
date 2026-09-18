import {Console} from "node:console";
import {pathToFileURL} from "node:url";
import distribution from "./bun.json";
import type {NyraContext} from "./context";

const args = process.argv.slice(2);
const writeProtocol = process.stdout.write.bind(process.stdout);
const exit = process.exit.bind(process);
let fatalExiting = false;
let commandHandler: ((message: unknown) => void) | undefined;

interface PendingCall {
    id: number;
    resolve: () => void;
    reject: (error: Error) => void;
}

interface Instruction {
    line: number;
    method: string;
    args: unknown[];
}

interface RunSession {
    runId: string;
    scriptPath: string;
    acceptingCalls: boolean;
    nextRequestId: number;
    instructions: Instruction[];
    pending?: PendingCall;
}

let activeRun: RunSession | undefined;

// stdout belongs exclusively to the lifecycle and SDK protocol.
Object.defineProperty(globalThis, "console", {
    value: new Console(process.stderr, process.stderr),
    writable: true,
    enumerable: true,
    configurable: true,
});

function errorText(error: unknown): string {
    return String(error instanceof Error ? error.stack ?? error.message : error).slice(0, 2000);
}

function writeMessage(message: object): Promise<void> {
    const line = JSON.stringify(message) + "\n";
    return new Promise<void>((resolve, reject) => {
        writeProtocol(line, (error) => error ? reject(error) : resolve());
    });
}

function stopPending(session: RunSession): void {
    if (!session.pending) return;
    const pending = session.pending;
    session.pending = undefined;
    pending.reject(new Error("Macro stopped before its instruction batch completed"));
}

function fatal(error: unknown): void {
    if (fatalExiting) return;
    fatalExiting = true;
    const session = activeRun;
    if (session) {
        session.acceptingCalls = false;
        stopPending(session);
        void writeMessage({
            version: 1,
            runId: session.runId,
            event: "failed",
            error: errorText(error),
        }).finally(() => exit(1));
    } else {
        console.error(errorText(error));
        exit(1);
    }
}

process.on("uncaughtException", fatal);
process.on("unhandledRejection", (error) => fatal(error ?? "Unhandled rejection"));
process.stdout.on("error", () => exit(1));
process.stderr.on("error", () => exit(1));
process.stdin.on("error", () => exit(1));
process.stdin.on("end", () => exit(1));

const started = new Promise<void>((resolve, reject) => {
    let input = "";
    let handshakeComplete = false;
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk: string) => {
        if (fatalExiting) return;
        try {
            input += chunk;
            let newline: number;
            while ((newline = input.indexOf("\n")) !== -1) {
                const line = input.slice(0, newline);
                input = input.slice(newline + 1);
                if (Buffer.byteLength(line, "utf8") > 16 * 1024) {
                    throw new Error("Host message exceeds 16 KiB");
                }
                if (!handshakeComplete) {
                    if (line !== "start") throw new Error("Invalid host handshake");
                    handshakeComplete = true;
                    resolve();
                    continue;
                }

                const message = JSON.parse(line);
                if (message?.event === "response") {
                    handleResponse(message);
                } else if (commandHandler) {
                    commandHandler(message);
                } else {
                    throw new Error("Host command arrived before the runner was ready");
                }
            }
            if (Buffer.byteLength(input, "utf8") > 16 * 1024) {
                throw new Error("Host message exceeds 16 KiB");
            }
        } catch (error) {
            reject(error);
            fatal(error);
        }
    });
    process.stdin.resume();
});

function handleResponse(response: any): void {
    const session = activeRun;
    if (!session || !response || response.version !== 1 || response.runId !== session.runId ||
        response.event !== "response" || !session.pending ||
        response.requestId !== session.pending.id || typeof response.ok !== "boolean" ||
        (response.ok ? response.error !== undefined : typeof response.error !== "string") ||
        Object.keys(response).some((key) => ![
            "version", "runId", "event", "requestId", "ok", "error",
        ].includes(key))) {
        throw new Error("Invalid SDK response from host");
    }
    const call = session.pending;
    session.pending = undefined;
    if (response.ok) call.resolve();
    else call.reject(new Error(response.error));
}

function captureSourceLine(scriptPath: string): number {
    const stack = new Error().stack;
    if (!stack) throw new Error("Unable to capture SDK instruction source line");
    const normalizedPath = scriptPath.replaceAll("\\", "/").toLowerCase();
    for (const frame of stack.split("\n").slice(1)) {
        const normalizedFrame = frame.replaceAll("\\", "/");
        const pathIndex = normalizedFrame.toLowerCase().indexOf(normalizedPath);
        if (pathIndex === -1) continue;
        const suffix = normalizedFrame.slice(pathIndex + normalizedPath.length);
        const location = /^:(\d+):\d+\)?$/.exec(suffix);
        if (!location) continue;
        const line = Number(location[1]);
        if (Number.isSafeInteger(line) && line > 0) return line;
    }
    throw new Error(`Unable to locate an SDK instruction call in ${scriptPath}`);
}

function enqueueInstruction(session: RunSession, method: string, args: unknown[]): void {
    if (activeRun !== session || !session.acceptingCalls || fatalExiting) {
        throw new Error("SDK instructions are not allowed after the macro entry returns");
    }
    session.instructions.push({
        line: captureSourceLine(session.scriptPath),
        method,
        args: structuredClone(args),
    });
}

function submitBatch(session: RunSession): Promise<void> {
    const id = session.nextRequestId++;
    return new Promise<void>((resolve, reject) => {
        session.pending = {id, resolve, reject};
        void writeMessage({
            version: 1,
            runId: session.runId,
            event: "batch",
            requestId: id,
            instructions: session.instructions,
        }).catch(fatal);
    });
}

function createContext(session: RunSession): NyraContext {
    const boundMethods = new Map<string, (...args: unknown[]) => void>();
    return new Proxy(Object.freeze({runId: session.runId}), {
        get(target, property, receiver) {
            if (Reflect.has(target, property)) return Reflect.get(target, property, receiver);
            if (typeof property !== "string") return undefined;
            let binding = boundMethods.get(property);
            if (!binding) {
                binding = (...args: unknown[]) => enqueueInstruction(session, property, args);
                boundMethods.set(property, binding);
            }
            return binding;
        },
        set: () => false,
    }) as NyraContext;
}

type Macro = (context: NyraContext) => void | Promise<void>;

async function loadMacro(scriptPath: string): Promise<Macro> {
    const module = await import(pathToFileURL(scriptPath).href);
    if (typeof module.default !== "function") {
        throw new Error(`${scriptPath}: default export must be a function`);
    }
    return module.default;
}

async function executeMacro(
    macro: Macro,
    runId: string,
    scriptPath: string,
): Promise<void> {
    const session: RunSession = {
        runId,
        scriptPath,
        acceptingCalls: true,
        nextRequestId: 1,
        instructions: [],
    };
    activeRun = session;
    let failure: unknown;
    try {
        const result = macro(createContext(session));
        await result;
        session.acceptingCalls = false;
        await new Promise<void>((resolve) => setImmediate(resolve));
        await submitBatch(session);
    } catch (error) {
        failure = error ?? "Macro failed";
        session.acceptingCalls = false;
        stopPending(session);
    }

    await writeMessage({
        version: 1,
        runId,
        event: failure === undefined ? "completed" : "failed",
        ...(failure === undefined ? {} : {error: errorText(failure)}),
    });
    activeRun = undefined;
}

async function runRunner(scriptPaths: string[]): Promise<void> {
    try {
        if (Bun.version !== distribution.version) {
            throw new Error(`Expected bundled Bun ${distribution.version}, got ${Bun.version}`);
        }
        const macros = await Promise.all(scriptPaths.map(loadMacro));
        commandHandler = (message: any) => {
            if (!message || message.version !== 1 || typeof message.event !== "string") {
                throw new Error("Invalid host command");
            }
            if (message.event === "shutdown" && Object.keys(message).length === 2) {
                exit(0);
            }
            if (message.event !== "run" || Object.keys(message).some((key) => ![
                    "version", "event", "runId", "scriptId",
                ].includes(key)) || typeof message.runId !== "string" ||
                !Number.isSafeInteger(message.scriptId) || !macros[message.scriptId]) {
                throw new Error("Invalid run command");
            }
            if (activeRun) throw new Error("A macro is already running");
            void executeMacro(
                macros[message.scriptId],
                message.runId,
                scriptPaths[message.scriptId],
            ).catch(fatal);
        };
        await writeMessage({
            version: 1,
            event: "ready",
            scriptCount: macros.length,
        });
    } catch (error) {
        await writeMessage({
            version: 1,
            event: "startupFailed",
            error: errorText(error),
        });
        exit(1);
    }
}

await started;
await runRunner(args);
