Take the first item from this list. Mark it as claimed in one commit, do the work, then remove it when done in another commit. This prevents any agent race conditions. If TODOS is empty, leave the heading in place l move onto QUESTIONS. For any of these, if the fix is clear/mechanistic, perform it autonomously as a single commit using /test-driven-development. If not, surface for discussion via the /brainstorming skill before acting.

# TODOS:

- Make sure all githooks are actually running on this machine
- Get the full test suite running on the full platform matrix in GitHub actions
- Update deps-check to run on all pushes as well
- Currently, if we add new files and start tracking them in one branch, they will not get caught by the branch-drift check. Let's supepower brainstorm solutions here: my thinking is that rather than doing opt-in tracking for all files, we should actually do a full list of all tracked files with an opt-in/opt-out label, so that new files that get tracked in one branch will throw, as well as any changes to files in either branch that don't get mirrored.
- Rename .my-scripts to just .scripts if possible. Make sure to sweep the repo for stale references

# QUESTIONS (leave until queried) 

- Are our git hooks currently configured to run the leak check on commit and then the test suite on push? If not, they should.
- Are we using the Docker container for the pre-push test suite? Should we be?
- Our testing & repo infrastructure has grown quite complex. Let's consider porting some of these to Rust scripts -- both for ease of reading/writing/updating/managing/testing, but also for speed
