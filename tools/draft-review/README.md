# draft-review

`draft-review` merges the JSON contributions of the code review skill and the Dev QA skill into one pending GitHub review. It never submits the review. The user reads the pending review on GitHub and submits it.

## Install

Run this command one time. It creates a link to the launcher in `~/.local/bin`, a folder on `PATH`, so the skills can run `draft-review` by name:

```sh
ln -s ~/tools/draft-review/bin/draft-review ~/.local/bin/draft-review
```

## Run it

Run `draft-review`, or `bin/draft-review` from this folder. The launcher finds Bun, installs the dependencies on the first run, and starts `src/main.ts`.

Every command takes these options:

- `--repo <owner/repo>` names the repository. Without it, `draft-review` asks `gh repo view` for the repository of the current folder.
- `--store <dir>` names the store root. The default is `~/.cache/draft-review`.

## Commands

### `draft-review add <contribution.json>`

Reads one contribution file, checks it against the contribution contract, and copies it into the store. The copy is a temporary file that a rename puts in place. When the store already holds a file of the same kind and platform for the same head, the new file replaces it. The command prints the path of the stored file.

Before `add` stores a `code-review` contribution, it removes the `dev-qa-verify*` files and `code-trace.json` of that head. Before it stores a `dev-qa` contribution, it removes `code-trace.json`. It also removes their `.files` records and names each removed file on stderr.

The command also records the SHA-256 hash of each file in the `uploads` list of the contribution. Later, `create` posts the content with that hash, not the content that the path holds at create time.

When the file is not a valid contribution, the command stores nothing and names the field path in the error. When a file in the `uploads` list does not exist, the command stores nothing, names the file, and exits with code 66.

Before it stores, `add` checks the merge rules on the set that the head holds after the add. When the set breaks a rule, the command stores nothing, names the rule, and exits with code 65.

`add` does not take the per-PR lock. Each stored file has its own name, and a rename puts each file in place, so two runs of `add` cannot mix their files.

Two orchestrators on one head at the same time can mix their contributions. Each `add` replaces the file of the same kind, so the round-two verdicts of one run can answer the findings of the other run. Run one orchestrator per head at a time.

### `draft-review route --pr <n> --head <sha> --out <dir>`

Reads the store for that head and writes two files in `<dir>`:

- `to-verify.json` holds the review findings that have a user-visible behavior. `/dev-qa --verify` reads this file.
- `to-trace.json` holds the Dev QA findings of kind `behavior`. `/expert-review-pr --trace` reads this file.

Each file has the shape `{"findings": [...]}`.

### `draft-review show --pr <n> --head <sha> [--config <file>]`

Prints the merged review: the body, each inline comment with its file and line, the local files to upload, and the hand-back lines. The text keeps local file paths in place of URLs. Use this output for the one approval question before `create`.

When the text holds a local path that `uploads` does not list, `show` ends with a section that names each such path and its location. `create` refuses that text.

When `draft.json` names a draft for another head, `show` ends with this line, and `create` prints it on stderr before it deletes that draft:

```text
create replaces the pending draft for head <short head>. Its text is in <path to synthesized.md of that head>.
```

`show` makes no GitHub call. So after the user submits that draft, `show` still prints the line. `create` never deletes a submitted review.

### `draft-review create --pr <n> --head <sha> [--config <file>]`

Takes the per-PR lock, merges the store, and creates the pending review. The steps are:

1. Read and merge the stored contributions for the head. Keep a digest of the contribution file names and contents, and of the `.files` hash record beside each contribution. When the body or a comment holds an absolute local path that `uploads` does not list, exit with code 65 and name the path and its location. A local path starts with `/Users/`, `/home/`, `/private/`, `/tmp/`, or `~/`.
2. Find the upload for each cited file by the hash that `add` recorded for it:
   - When the file holds the recorded content, reuse its upload from the ledger, or upload it in step 5.
   - When the file changed after `add`, or no longer exists, use the ledger upload of the recorded content. When the ledger has none, exit with code 66. For a changed file, the message says `<path> changed after add, run add again`.
   - When two contributions for the head recorded different content for one path, exit with code 66.
   - When `add` recorded no hash for the path, use the content that the path holds now.
3. Check each inline comment against the pull-request diff. The command reads `GET repos/{owner}/{repo}/pulls/{pr}/files` and takes the line ranges of each hunk from the `patch` of each file. The `line` of the comment must sit inside one hunk on the side of the comment, together with its optional `startLine`. When a comment fails, the command uploads nothing, deletes nothing, and exits with code 65. The message names the file, the line, and the contribution that holds the comment. A file that GitHub sends with no `patch` fails every comment on it.
4. Find the pending reviews of the signed-in user on the pull request, with the inline comments of each review. Read the review that `draft.json` names by its id, and get its state. Then decide:
   - When the user submitted that review, and `draft.json` names the same head, change nothing. Write the merged review to `synthesized.md`, print the path, and exit with code 3. See [A submitted first draft](#a-submitted-first-draft).
   - When GitHub returns `404` for that review, the command ignores `draft.json` for this decision.
   - When a submitted review by the user is for the head and holds the body that the command is about to send, change nothing. Write the merged review to `synthesized.md`, print the path, and exit with code 3. This check does not use `draft.json`.
   - When a pending review is not one that `draft-review` can replace, change nothing. Write the merged review to `synthesized.md`, print the path, and exit with code 3.
5. Upload each file from step 2. For each file, the command reads the file one time, hashes those bytes, and uploads the same bytes. When the hash is not the hash from step 2, the file changed while `create` ran. The command then uploads nothing more, says `<path> changed while create ran. Run create again.`, and exits with code 66. The command writes the ledger after each upload, so a failed run keeps the URLs it already has.
6. Put the URLs in place of the local paths. Write the merged review to `synthesized.md` and the exact request to `payload.request`.
7. Find the pending reviews again, and decide again.
8. When no pending review exists, create one.
9. When the only pending review is the review that `draft-review` created last, delete that review and create the new one. The body and every inline comment must be unchanged. An added, an edited, or a deleted inline comment makes the review one that `draft-review` cannot replace.
10. When any other pending review exists, change nothing, and exit with code 3.
11. When the user submitted a review after step 4, and step 4 refuses that review now, change nothing, and exit with code 3.

On success, the command prints the URL of the pending review. Then it reads the inline comments of the new review back from GitHub and compares them with the request. The comparison changes CRLF to LF and removes trailing whitespace from each body on both sides. When the count or the set of comment bodies differs, a comment can be the user's work. The command then writes no `draft.json`, says why, and exits with code 3. When the read fails, the command writes no `draft.json`, says why, and exits with code 69. Without `draft.json`, the next `create` refuses to replace that review.

When the comments match, the command writes `draft.json`. Then it reads the contribution files for the head again. When the digest from step 1 changed, an `add` landed while `create` ran, and the new draft does not hold it. The command says that the draft is stale and exits with code 75. Run `create` again: it replaces the stale draft.

When the create fails after the delete, the command names the deleted review and the path of `payload.request`, and exits with code 69. A GitHub `422` reply repeats on every `create` until a contribution changes. For any other cause, run `create` again to make the draft again.

The request never names an `event`, so GitHub keeps the review pending.

### `draft-review clear --pr <n> --head <sha>`

Takes the per-PR lock and removes the stored contributions for that head. An orchestrator runs `clear` before a new run on a head, so that the contributions of an earlier run do not enter the new draft.

The command removes each file in the head folder whose name matches the contribution file grammar. It also removes the `.files` record of each of those names, and a `.files` record whose contribution is gone. The command keeps every other file: `uploads.json`, `draft.json`, `synthesized.md`, `payload.request`, and every file outside the head folder.

The command prints the name of each removed file, one name per line. When the head folder holds no contribution file, the command prints nothing and exits with code 0. A missing head folder also gives code 0. Without `--repo`, the command asks `gh repo view` for the repository. It makes no other call to GitHub.

## Pending reviews that `create` never changes

GitHub allows one pending review per user on a pull request. `create` replaces only the review that it created last and that nobody edited. These GitHub facts explain why `create` never appends to another pending review:

- REST cannot add a comment to an existing pending review. `POST repos/{owner}/{repo}/pulls/{pr}/reviews/{review_id}/comments` returns `404`. `POST repos/{owner}/{repo}/pulls/{pr}/comments` returns `422` with `user_id can only have one pending review per pull request`.
- Comments of an unsubmitted review are invisible to REST. `GET repos/{owner}/{repo}/pulls/comments/{comment_id}` returns `404` for them. `GET repos/{owner}/{repo}/pulls/{pr}/reviews/{review_id}/comments` returns them to the review's author.
- The body of a pending review belongs to its author. Text that the tool wrote there publishes under that author's name.

When another pending review exists, `create` exits with code 3 and writes the merged review to `synthesized.md`. The user submits or discards the other review, then runs `create` again.

## A submitted first draft

The user can submit the pending review that `draft-review` created. The submitted review is public, so `create` must never post its content again.

When `draft.json` names a submitted review for the same head, `create` changes nothing on GitHub. It writes the merged review to `synthesized.md`, prints the path on stdout, and exits with code 3. The message on stderr is:

```text
The first draft for head <head> (review <id>) was submitted, and its state is <state>. draft-review never posts submitted content again. The merged review is in <path>.
```

Every later `create` for that head gives the same result. To add new findings to the pull request, the user copies them from `synthesized.md` by hand.

`create` reads the state from `GET repos/{owner}/{repo}/pulls/{pr}/reviews/{review_id}`. Every state other than `PENDING` counts as submitted. The GitHub documentation names `COMMENTED`, `APPROVED`, `CHANGES_REQUESTED`, and `DISMISSED`. A state that GitHub adds later also counts as submitted.

A submitted review for an earlier head does not stop `create`. A new head starts a new round of review, so `create` makes a new draft for the new head.

When GitHub returns `404` for the review, the user deleted the draft, or the signed-in user cannot see that review. `create` then decides without `draft.json`.

`create` has a second check that does not use `draft.json`. Three events leave `draft.json` absent or stale: a failed read-back, a run that stopped before it wrote `draft.json`, and a delete of `draft.json` by the user. The check reads each review by the signed-in user that is not `PENDING`. It compares the `commit_id` with the head, and the body with the body that `create` is about to send. The comparison changes CRLF to LF and removes trailing whitespace on both sides. When both match, `create` changes nothing on GitHub, writes `synthesized.md`, prints the path, and exits with code 3. The message on stderr is:

```text
A submitted review for head <head> (review <id>, state <state>) already holds this content. draft-review never posts submitted content again. The merged review is in <path>.
```

The same contributions on the same head give the same body. The body holds the ledger URLs, and the ledger keeps one URL for each content hash.

The check runs after the upload plan. So a missing cited file (code 66) or a missing `uploadEndpoint` (code 78) stops `create` before either submitted check. In that case, `create` posts nothing.

## Exit codes

| Code | Name | Meaning |
|---|---|---|
| 0 | `ok` | The command finished. |
| 3 | `refused` | Another pending review by the user exists, the user edited the review that `draft-review` created, the user submitted the first draft for the head, a submitted review for the head holds the same body, or the comments of a new review do not match the request. When no review URL was printed, the merged review is in `synthesized.md`. When a review URL was printed, a new pending review exists with no `draft.json`, and the user must check it on GitHub and delete it before the next `create`. |
| 64 | `usage` | The command line is wrong. |
| 65 | `dataErr` | A contribution is invalid, the stored contributions break a merge rule, an inline comment is outside the pull-request diff, the text holds a local path that `uploads` does not list, or `draft.json` is invalid. See [An invalid draft.json](#an-invalid-draftjson). |
| 66 | `noInput` | An input file is missing, a cited file is missing or changed after `add` with no upload of its recorded content, a cited file changed while `create` ran, two contributions recorded different content for one file, or the store holds nothing for the head. |
| 69 | `unavailable` | `gh` is missing or not signed in, or a GitHub call or an upload failed. This code includes a create that failed after the delete, and a failed read of the new review's comments. When a review URL was printed, a pending review exists with no `draft.json`. The user must check it on GitHub and delete it before the next `create`. |
| 73 | `cantCreate` | Another run holds the per-PR lock for longer than 30 seconds. |
| 74 | `ioErr` | A store file cannot be read or written. |
| 75 | `tempFail` | `create` made the pending review, but a contribution for the head changed while it ran. Run `create` again. |
| 78 | `config` | The config file is missing or invalid, it has no entry for the repository, or an upload needs an `uploadEndpoint` that the entry does not set. |

## An invalid draft.json

When `draft.json` is not JSON, or does not match the schema, `create` exits with code 65. The message names the file and the problem. When the file holds a `reviewId`, the message names that review:

```text
<path> is invalid: <problem>. Fix it or delete it. It names review <id>. A delete of draft.json is safe: create never posts the content of a submitted review again. While review <id> is pending, the next create refuses to replace it. Delete review <id> on GitHub to unblock create.
```

A delete of `draft.json` cannot cause a second post of submitted content, because the content check does not use `draft.json`. But without `draft.json`, `create` cannot tell its own draft from a review that the user wrote. So it refuses every pending review, and the user must delete the old draft on GitHub.

A `draft.json` from an older round has no `contributionsDigest`. `create` loads it.

## Store layout

```text
<store root>/<owner>/<repo>/<pr>/
  .lock                     the per-PR lock file (flock)
  uploads.json              the upload ledger: {"entries": [{path, sha256, url}]}
  draft.json                the last review that draft-review created: {reviewId, bodyHash, comments: [{id, bodyHash}], contributionsDigest (optional), head, url}
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

Each contribution file has a `.files` file beside it. The tree shows only one of them. The commands read a `.files` file only beside a contribution file that exists and has a contribution file name. An `add` that stops between its two writes leaves a `.files` file with no contribution, and the commands ignore it.

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

- The pure core: `add-plan.ts`, `clear.ts`, `contribution.ts`, `contribution-set.ts`, `diff-anchors.ts`, `synthesize.ts`, `uploads.ts`, `payload.ts`, `parsed.ts`, `lists.ts`, `args.ts`, `draft.ts`, `ledger.ts`, `render.ts`, and `exit.ts`. These files do no IO.
- The shell: `main.ts` (the commands), `store.ts` (the store files and the upload loop), `github.ts` (every `gh` call), `lock.ts` (the lock), and `config.ts` (the config file).

The test `src/core-imports.test.ts` holds that boundary. A core file can import only core files, `zod`, `node:crypto`, and `node:util`. It cannot name `Bun` or `process`, it cannot call `require`, and it cannot read the clock with `Date.now` or `new Date(`. The test removes comments and string literals before it looks for these names, so a comment or a message can use the words.

The one seam is `IUploader` in `store.ts`. The upload loop gives the uploader the path and the bytes of each file to upload, and the uploader returns the URL. A test can pass a fake uploader.

## Development

```sh
bun install
bun run check
bun run test
```
