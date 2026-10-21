export type Parsed<Value> = {ok: true; value: Value} | {ok: false; reason: string}
