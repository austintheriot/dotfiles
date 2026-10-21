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

The command also records the SHA-256 hash of each file in the `uploads` list of the contribution. Later, `create` posts the content with that hash, not the content that the path holds at create time.

When the file is not a valid contribution, the command stores nothing and names the field path in the error. When a file in the `uploads` list does not exist, the command stores nothing, names the file, and exits with code 66.

`add` does not take the per-PR lock. Each stored file has its own name, and a rename puts each file in place, so two runs of `add` cannot mix their files.

### `draft-review route --pr <n> --head <sha> --out <dir>`

Reads the store for that head and writes two files in `<dir>`:

- `to-verify.json` holds the review findings that have a user-visible behavior. `/dev-qa --verify` reads this file.
- `to-trace.json` holds the Dev QA findings of kind `behavior`. `/expert-review-pr --trace` reads this file.

Each file has the shape `{"findings": [...]}`.

### `draft-review show --pr <n> --head <sha> [--config <file>]`

Prints the merged review: the body, each inline comment with its file and line, the local files to upload, and the hand-back lines. The text keeps local file paths in place of URLs. Use this output for the one approval question before `create`.

### `draft-review create --pr <n> --head <sha> [--config <file>]`

Takes the per-PR lock, merges the store, and creates the pending review. The steps are:

1. Read and merge the stored contributions for the head. Keep a digest of the contribution file names and contents.
2. Find the upload for each cited file by the hash that `add` recorded for it:
   - When the file holds the recorded content, reuse its upload from the ledger, or upload it in step 4.
   - When the file changed after `add`, or no longer exists, use the ledger upload of the recorded content. When the ledger has none, exit with code 66. For a changed file, the message says `<path> changed after add, run add again`.
   - When two contributions for the head recorded different content for one path, exit with code 66.
   - When `add` recorded no hash for the path, use the content that the path holds now.
3. Find the pending reviews of the signed-in user on the pull request, with the inline comments of each review. When a pending review is not one that `draft-review` can replace, change nothing. Write the merged review to `synthesized.md`, print the path, and exit with code 3.
4. Upload each file from step 2. The command writes the ledger after each upload, so a failed run keeps the URLs it already has.
5. Put the URLs in place of the local paths. Write the merged review to `synthesized.md` and the exact request to `payload.request`.
6. Find the pending reviews again, and decide again.
7. When no pending review exists, create one.
8. When the only pending review is the review that `draft-review` created last, delete that review and create the new one. The body and every inline comment must be unchanged. An added, an edited, or a deleted inline comment makes the review one that `draft-review` cannot replace.
9. When any other pending review exists, change nothing, and exit with code 3.

On success, the command prints the URL of the pending review. Then it reads the inline comments of the new review back from GitHub and compares them with the request. When the count or the set of comment bodies differs, a comment can be the user's work. The command then writes no `draft.json`, says why, and exits with code 3. When the read fails, the command writes no `draft.json`, says why, and exits with code 69. Without `draft.json`, the next `create` refuses to replace that review.

When the comments match, the command writes `draft.json`. Then it reads the contribution files for the head again. When the digest from step 1 changed, an `add` landed while `create` ran, and the new draft does not hold it. The command says that the draft is stale and exits with code 75. Run `create` again: it replaces the stale draft.

When the create fails after the delete, the command names the deleted review and the path of `payload.request`, and exits with code 69. Run `create` again to make the draft again.

The request never names an `event`, so GitHub keeps the review pending.

## Exit codes

| Code | Name | Meaning |
|---|---|---|
| 0 | `ok` | The command finished. |
| 3 | `refused` | Another pending review by the user exists, the user changed the review that `draft-review` created, or the comments of a new review do not match the request. The merged review is in `synthesized.md`. |
| 64 | `usage` | The command line is wrong. |
| 65 | `dataErr` | A contribution is invalid, or the stored contributions break a merge rule. |
| 66 | `noInput` | An input file is missing, a cited file is missing or changed after `add` with no upload of its recorded content, two contributions recorded different content for one file, or the store holds nothing for the head. |
| 69 | `unavailable` | `gh` is missing or not signed in, or a GitHub call or an upload failed. This code includes a create that failed after the delete, and a failed read of the new review's comments. |
| 73 | `cantCreate` | Another run holds the per-PR lock for longer than 30 seconds. |
| 74 | `ioErr` | A store file cannot be read or written. |
| 75 | `tempFail` | `create` made the pending review, but a contribution for the head changed while it ran. Run `create` again. |
| 78 | `config` | The config file is missing or invalid, it has no entry for the repository, or an upload needs an `uploadEndpoint` that the entry does not set. |

## Store layout

```text
<store root>/<owner>/<repo>/<pr>/
  .lock                     the per-PR lock file (flock)
  uploads.json              the upload ledger: {"entries": [{path, sha256, url}]}
  draft.json                the last review that draft-review created: {reviewId, bodyHash, comments: [{id, bodyHash}], contributionsDigest, head, url}
  <head>/
    code-review.json
    code-review.json.files  the hashes that add recorded: {"files": [{path, sha256}]}
    dev-qa.json
    dev-qa@<platform>.json
    dev-qa-verify.json
    dev-qa-verify@<platform>.json
    code-trace.json
    synthesized.md          the merged review as text
    payload.request         the exact request that create sends to GitHub, as JSON
```

Each contribution file has a `.files` file beside it. The tree shows only one of them.

A contribution file name starts with the kind. When the contribution has a platform, the name adds `@` and the platform. The name encodes the platform with `encodeURIComponent`. A platform label never contains `@`, so two kinds cannot share a file name. The commands read only files whose names match `^(code-review|dev-qa|dev-qa-verify|code-trace)(@[^/]+)?\.json$` as contributions. They ignore every other file in the head folder.

The ledger keys each upload by the SHA-256 hash of the file content. A file with new content at a known path gets a new upload. `create` finds the upload of a cited file by the hash that `add` recorded for that head. The path alone never selects an upload, because an earlier head can have a different file at the same path.

The writes do not call `fsync`. After a power loss, a store file can hold old content.

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

The test `src/core-imports.test.ts` holds that boundary. A core file can import only core files, `zod`, `node:crypto`, and `node:util`. It cannot name `Bun` or `process`, and it cannot call `require`.

The one seam is `IUploader` in `store.ts`. The upload loop asks the uploader for each missing URL, so a test can pass a fake uploader.

## Development

```sh
bun install
bun run check
bun run test
```
