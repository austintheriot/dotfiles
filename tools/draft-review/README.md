# draft-review

`draft-review` merges the JSON contributions of the code review skill and the Dev QA skill into one pending GitHub review. It never submits the review. The user reads the pending review on GitHub and submits it.

## Run it

Run `bin/draft-review`. A symbolic link to that file in a folder on `PATH` also works. The launcher finds Bun, installs the dependencies on the first run, and starts `src/main.ts`.

Every command takes these options:

- `--repo <owner/repo>` names the repository. Without it, `draft-review` asks `gh repo view` for the repository of the current folder.
- `--store <dir>` names the store root. The default is `~/.cache/draft-review`.

## Commands

### `draft-review add <contribution.json>`

Reads one contribution file, checks it against the contribution contract, and copies it into the store. The copy is a temporary file that a rename puts in place. When the store already holds a file of the same kind and platform for the same head, the new file replaces it. The command prints the path of the stored file.

When the file is not a valid contribution, the command stores nothing and names the field path in the error.

### `draft-review route --pr <n> --head <sha> --out <dir>`

Reads the store for that head and writes two files in `<dir>`:

- `to-verify.json` holds the review findings that have a user-visible behavior. `/dev-qa --verify` reads this file.
- `to-trace.json` holds the Dev QA findings of kind `behavior`. `/expert-review-pr --trace` reads this file.

Each file has the shape `{"findings": [...]}`.

### `draft-review show --pr <n> --head <sha> [--config <file>]`

Prints the merged review: the body, each inline comment with its file and line, the local files to upload, and the hand-back lines. The text keeps local file paths in place of URLs. Use this output for the one approval question before `create`.

### `draft-review create --pr <n> --head <sha> [--config <file>]`

Takes the per-PR lock, merges the store, and creates the pending review. The steps are:

1. Merge the stored contributions for the head.
2. Upload each cited file that the upload ledger does not hold. The command writes the ledger after each upload, so a failed run keeps the URLs it already has.
3. Put the URLs in place of the local paths.
4. Find the pending reviews of the signed-in user on the pull request.
5. When no pending review exists, create one.
6. When the only pending review is the review that `draft-review` created last, and its body is unchanged, delete that review and create the new one.
7. When any other pending review exists, change nothing. Write the merged review to `synthesized.md` in the store, print the path, and exit with code 3.

On success, the command prints the URL of the pending review.

## Exit codes

| Code | Name | Meaning |
|---|---|---|
| 0 | `ok` | The command finished. |
| 3 | `refused` | Another pending review by the user exists. The merged review is in `synthesized.md`. |
| 64 | `usage` | The command line is wrong. |
| 65 | `dataErr` | A contribution is invalid, or the stored contributions break a merge rule. |
| 66 | `noInput` | An input file is missing, or the store holds nothing for the head. |
| 69 | `unavailable` | `gh` is missing or not signed in, or a GitHub call or an upload failed. |
| 73 | `cantCreate` | Another run holds the per-PR lock for longer than 30 seconds. |
| 74 | `ioErr` | A store file cannot be read or written. |
| 78 | `config` | The config file is missing or invalid, it has no entry for the repository, or an upload needs an `uploadEndpoint` that the entry does not set. |

## Store layout

```text
<store root>/<owner>/<repo>/<pr>/
  .lock                     the per-PR lock file (flock)
  uploads.json              the upload ledger: {"entries": [{path, sha256, url}]}
  draft.json                the last review that draft-review created: {reviewId, bodyHash, head, url}
  <head>/
    code-review.json
    dev-qa.json
    dev-qa@<platform>.json
    dev-qa-verify.json
    dev-qa-verify@<platform>.json
    code-trace.json
    synthesized.md          written only when create refuses
```

A contribution file name starts with the kind. When the contribution has a platform, the name adds `@` and the platform. The name encodes the platform with `encodeURIComponent`. A platform label never contains `@`, so two kinds cannot share a file name.

The ledger keys each upload by the SHA-256 hash of the file content. A file with new content at a known path gets a new upload. When a cited file no longer exists, the ledger gives the last URL for that path.

The ledger and `draft.json` belong to the pull request, not to one head. So a run for a new head can replace the pending review that a run for an earlier head created.

## Config

The default config file is `~/.claude/local/draft-review.json`. Pass `--config <file>` to use another file. Example with neutral values:

```json
{
  "repositories": {
    "example-org/example-repo": {
      "uploadEndpoint": "https://uploads.example.com/assets",
      "repositoryId": 12345,
      "attribution": "Posted by a bot on behalf of @example-user"
    }
  }
}
```

Each entry under `repositories` has three optional fields:

- `uploadEndpoint` is the URL that receives each file upload. `draft-review` sends the file with `gh api --method POST` and adds the query parameters `name` and `content_type`. The reply must be `{"url": "..."}`. When a cited file needs an upload, the run needs this field. Otherwise the run does not read it.
- `repositoryId` is a number. When it is set, each upload also sends it as the `repository_id` query parameter.
- `attribution` is the line that starts the body and each inline comment.

`create` needs the config file and an entry for the repository. `show` works without them, but it then shows no attribution line.

## Contribution contract

The contribution contract is the `## Orchestrated mode` section of `.claude/rules/outside-pr-review.md`. That section is the only definition of the contract. The test `src/contract-agreement.test.ts` reads the field tables in that section and compares them with the schemas in `src/contribution.ts`. When a field changes on one side only, the test fails.

## Layout of the code

- The pure core: `contribution.ts`, `contribution-set.ts`, `synthesize.ts`, `uploads.ts`, `payload.ts`, `parsed.ts`, `lists.ts`, `args.ts`, `draft.ts`, `ledger.ts`, `render.ts`, and `exit.ts`. These files do no IO.
- The shell: `main.ts` (the commands), `store.ts` (the store files and the upload loop), `github.ts` (every `gh` call), `lock.ts` (the lock), and `config.ts` (the config file).

The one seam is `IUploader` in `store.ts`. The upload loop asks the uploader for each missing URL, so a test can pass a fake uploader.

## Development

```sh
bun install
bun run check
bun run test
```
