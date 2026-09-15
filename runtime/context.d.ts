/** Context supplied to the default async macro entry point. */
export interface NyraContext {
    readonly runId: string;
    /** Writes through the Rust host; await its acknowledgement before continuing. */
    log(message: string): Promise<void>;
}
