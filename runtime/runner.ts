import {Console} from "node:console";
import {readSync, writeSync} from "node:fs";
import {pathToFileURL} from "node:url";
import distribution from "./bun.json";
import {createLocalMethods, type NyraContext} from "./sdk";

const args = process.argv.slice(2);
const exit: (code: number) => never = process.exit.bind(process);
const readBuffer = Buffer.allocUnsafe(4096);
let input = Buffer.alloc(0);
let fatalExiting = false;

interface RunSession {
    runId: string;
    scriptPath: string;
    acceptingCalls: boolean;
    nextRequestId: number;
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

function writeMessage(message: object): void {
    const bytes = Buffer.from(JSON.stringify(message) + "\n");
    let offset = 0;
    while (offset < bytes.length) {
        const written = writeSync(1, bytes, offset, bytes.length - offset);
        if (written === 0) throw new Error("Host output channel has been closed");
        offset += written;
    }
}

function readLine(): string {
    while (true) {
        const newline = input.indexOf(10);
        if (newline !== -1) {
            if (newline > 16 * 1024) throw new Error("Host message exceeds 16 KiB");
            const line = input.subarray(0, newline).toString("utf8");
            input = input.subarray(newline + 1);
            return line.replace(/\r$/, "");
        }
        if (input.length > 16 * 1024) throw new Error("Host message exceeds 16 KiB");
        const bytesRead = readSync(0, readBuffer, 0, readBuffer.length, null);
        if (bytesRead === 0) throw new Error("Host input channel has been closed");
        input = Buffer.concat([input, readBuffer.subarray(0, bytesRead)]);
    }
}

function fatal(error: unknown): never {
    if (fatalExiting) exit(1);
    fatalExiting = true;
    try {
        const session = activeRun;
        if (session) {
            session.acceptingCalls = false;
            writeMessage({
                version: 2,
                runId: session.runId,
                event: "failed",
                error: errorText(error),
            });
        } else {
            console.error(errorText(error));
        }
    } finally {
        exit(1);
    }
}

process.on("uncaughtException", fatal);
process.on("unhandledRejection", (error) => fatal(error ?? "Unhandled rejection"));
process.stderr.on("error", () => exit(1));

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

function callHost(session: RunSession, method: string, args: unknown[]): void {
    if (activeRun !== session || !session.acceptingCalls || fatalExiting) {
        throw new Error("SDK calls are not allowed after the macro entry returns");
    }
    const id = session.nextRequestId;
    writeMessage({
        version: 2,
        runId: session.runId,
        event: "call",
        requestId: id,
        instruction: {line: captureSourceLine(session.scriptPath), method, args},
    });
    session.nextRequestId += 1;
    let response: any;
    try {
        response = JSON.parse(readLine());
        if (!response || response.version !== 2 || response.runId !== session.runId ||
            response.event !== "response" || response.requestId !== id ||
            typeof response.ok !== "boolean" ||
            (response.ok ? response.error !== undefined : typeof response.error !== "string") ||
            Object.keys(response).some((key) => ![
                "version", "runId", "event", "requestId", "ok", "error",
            ].includes(key))) {
            throw new Error("Invalid SDK response from host");
        }
    } catch (error) {
        fatal(error);
    }
    if (!response.ok) throw new Error(response.error);
}

function createContext(session: RunSession): NyraContext {
    const localMethods = createLocalMethods(
        (point, options) => callHost(session, "click", [point, options]),
        (durationMs) => callHost(session, "delay", [durationMs]),
    );
    const boundMethods = new Map<string, (...args: unknown[]) => void>();
    return new Proxy(Object.freeze({runId: session.runId}), {
        get(target, property, receiver) {
            if (Reflect.has(target, property)) return Reflect.get(target, property, receiver);
            if (Reflect.has(localMethods, property)) {
                return Reflect.get(localMethods, property, localMethods);
            }
            if (typeof property !== "string") return undefined;
            let binding = boundMethods.get(property);
            if (!binding) {
                binding = (...args: unknown[]) => callHost(session, property, args);
                boundMethods.set(property, binding);
            }
            return binding;
        },
        set: () => false,
    }) as NyraContext;
}

type Macro = (context: NyraContext) => unknown;

async function loadMacro(scriptPath: string): Promise<Macro> {
    const module = await import(pathToFileURL(scriptPath).href);
    if (typeof module.default !== "function" || module.default.constructor.name === "AsyncFunction") {
        throw new Error(`${scriptPath}: default export must be a synchronous function`);
    }
    return module.default;
}

function executeMacro(macro: Macro, runId: string, scriptPath: string): void {
    const session: RunSession = {
        runId,
        scriptPath,
        acceptingCalls: true,
        nextRequestId: 1,
    };
    activeRun = session;
    let failure: unknown;
    try {
        const result = macro(createContext(session));
        if (result && typeof (result as PromiseLike<unknown>).then === "function") {
            throw new Error("Macro entry must not return a Promise; use synchronous SDK calls");
        }
    } catch (error) {
        failure = error ?? "Macro failed";
    } finally {
        session.acceptingCalls = false;
    }

    writeMessage({
        version: 2,
        runId,
        event: failure === undefined ? "completed" : "failed",
        ...(failure === undefined ? {} : {error: errorText(failure)}),
    });
    activeRun = undefined;
}

async function runRunner(scriptPaths: string[]): Promise<void> {
    let macros: Macro[];
    try {
        if (Bun.version !== distribution.version) {
            throw new Error(`Expected bundled Bun ${distribution.version}, got ${Bun.version}`);
        }
        macros = await Promise.all(scriptPaths.map(loadMacro));
    } catch (error) {
        writeMessage({version: 2, event: "startupFailed", error: errorText(error)});
        exit(1);
    }
    writeMessage({version: 2, event: "ready", scriptCount: macros.length});
    while (true) {
        const message = JSON.parse(readLine());
        if (!message || message.version !== 2 || typeof message.event !== "string") {
            throw new Error("Invalid host command");
        }
        if (message.event === "shutdown" && Object.keys(message).length === 2) return;
        if (message.event !== "run" || Object.keys(message).some((key) => ![
                "version", "event", "runId", "scriptId",
            ].includes(key)) || typeof message.runId !== "string" ||
            !Number.isSafeInteger(message.scriptId) || !macros[message.scriptId]) {
            throw new Error("Invalid run command");
        }
        executeMacro(macros[message.scriptId], message.runId, scriptPaths[message.scriptId]);
    }
}

try {
    if (readLine() !== "start") throw new Error("Invalid host handshake");
    await runRunner(args);
} catch (error) {
    fatal(error);
}
