// BSD exit codes from sysexits.h, plus 3 for a refusal that leaves the body on disk for the user.
export const EXIT = {ok: 0, refused: 3, usage: 64, dataErr: 65, noInput: 66, unavailable: 69, cantCreate: 73, ioErr: 74, tempFail: 75, config: 78} as const

export type ExitCode = (typeof EXIT)[keyof typeof EXIT]

export class DraftReviewError extends Error {
  public constructor(
    message: string,
    public readonly exitCode: ExitCode
  ) {
    super(message)
  }
}
