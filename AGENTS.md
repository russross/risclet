HARD RULES
==========

*   `git` commands that make changes require permission
    *   Always display a proposed commit message before asking permission to commit
    *   Never `git reset` or `git checkout`, etc., unless requested by the user
        *   Need to see an old version? Use `git diff`
        *   Need to undo changes? Undo directly, or use `git diff` and
            then carefully apply changes to restore an old state
*   If you want to deviate from a plan or deviate from instructions, you must
    ALWAYS ask first, and always explain the steps that led you to propose a
    change before asking
*   Never claim code you have written is "production ready" or anything similar
*   Always run tests after changes, but only when you expect them to pass.
    *   If you expect tests to fail, always report the expectation before
        running the tests
*   When a unit test fails unexpectedly, you must always:
    *   Read the complete test
    *   Reason about what the test is designed to check, i.e., which feature it
        is measuring (not just the mechanics of what it does), and report this
        high-level understanding to the user.
    *   Tests should be well commented and well named. When you discover a
        mismatch between how the test is described and what it actually tests,
        report this to the user, propose a fix, and ask permission to continue.
    *   If the failed test correctly identifies a bug, you should prioritize
        fixing the bug.
    *   Never change what a test measures without permission.
        *   If you are refactoring and the test requires refactoring to continue
            working, you may always do this, but you should verify afterward
            that it is testing the same thing it did before, just using the
            newly-refactored interface, etc.
    *   If you identify a test that was written incorrectly or that tests the
        wrong thing, always report, propose a solution to the user, and ask for
        permission.
