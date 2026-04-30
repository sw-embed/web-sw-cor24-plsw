# Mass Compile

Mass Compile runs several demo programs as a queued batch. It is intended for
quick regression-style checks across examples without manually selecting and
running one demo at a time.

## Motivation

The MVS/390 PL/EDIT workflow was built around turnaround time. A programmer
could submit work to a compile queue, return immediately to the editor, and keep
changing another source member while earlier jobs compiled, assembled, and ran.
That mattered because waiting to finish all edits before submitting meant other
jobs could enter the queue first.

This browser version demonstrates the same idea in miniature:

- Submit several demo compile jobs, either one line at a time or all at once.
- Continue editing source while the earlier jobs run.
- Let queued jobs pick up the latest draft when they actually reach compile
  time.
- Use per-job scratch source when multiple queued instances of the same demo
  should compile different edits.
- Hold the queue at `waiting` when a dirty scratch buffer is still being edited,
  matching the old write-lock/read-lock handoff in spirit.
- Inspect completed output logs while a later job is still compiling.

The goal is not only batch convenience. The important behavior is overlapping
editing with the compile queue, without mutating the original bundled demo
source.

## Opening The Dialog

Use the `Mass Compile` button in the PL/SW source editor action row, after the
fullscreen button. The dialog contains two panels:

- Left panel: job entry form before submit, then job list and states
- Right panel: selected job output log

## Creating A Batch

1. Add one row for each job.
2. Select a demo in each row.
3. Select a row to view or edit its draft source scratch.
4. Press `Save` if the scratch buffer is dirty.
5. Press `Submit` on an individual row, or `Submit All`.

The same demo may be selected more than once. Jobs run sequentially so only one
job is active at a time. Closing the dialog hides the batch view but does not
stop submitted jobs; reopen `Mass Compile` to inspect progress.

## Queued Source Drafts

Submitted jobs do not modify the bundled demo source. They compile from browser
drafts:

- If a queued job has no scratch edit, it snapshots the current draft for that
  demo when the job enters `compiling`.
- If the same demo is queued several times, each row snapshots independently
  when that row starts compiling.
- Selecting a draft row before submit, or a queued job after submit, shows a
  source scratch editor in the left panel. Edits there apply only to that row.
- Scratch edits are dirty until `Save` is pressed. Dirty text is not compiler
  input yet.
- If a queued job reaches the head of the queue while its scratch editor is
  dirty, its state changes to flashing `waiting` and the queue stops there until
  `Save` is pressed.
- Editing a later queued job does not change its state to `waiting` and does
  not pause the active compile. It remains `queued` with an unsaved scratch note
  until its own turn arrives.
- Scratch edits affect only queued jobs. Once a job begins compiling, that job's
  source is fixed for the compile/assemble/run cycle and the scratch editor is
  disabled.
- Completed and failed jobs can be edited again. Save the scratch edit, then
  press that row's `Submit` button to rerun only that line.
- Completed and failed rows also re-enable the demo selector. Changing the demo
  turns that row into a reusable job slot with fresh scratch source for the new
  demo.

## Submitting And Resubmitting

Before a batch exists, each draft row has a `Submit` button and the form has a
`Submit All` button.

After jobs are submitted:

- A completed or failed row gets its own `Submit` button.
- A completed or failed row can be changed to a different demo before it is
  resubmitted.
- Pressing a row's `Submit` resets only that row to `queued`.
- Pressing `Submit All` after all jobs complete or fail resets every row to
  `queued`.
- Active jobs cannot be edited or resubmitted until they finish.

## Job States

Each submitted job moves through these states:

- `queued`
- `waiting`
- `compiling`
- `assembling`
- `running`
- `complete`
- `failed`

If a job fails, the batch continues with the next queued job.

## Job Logs

Select any job in the left panel to view its log in the right panel. The log is
available while the job runs and after it completes or fails. It shows as much
output as the job has produced:

- Compiler input, including FILE/SOURCE blocks
- Compiler output
- Generated assembly
- Assembler object listing
- Assembler errors, if any
- Run output
- Run errors, if any

The log selection is manual. Starting the next queued job does not switch the
right panel away from the job you selected.

During the compile state, Mass Compile runs the emulator in cooperative batches
and yields back to the browser between groups of batches. This keeps completed
job logs selectable while a later job is compiling.

The state machine also leaves a short visible gap between active stages so
`assembling` and `running` can be seen in the job list instead of collapsing
directly from `compiling` to `complete`.
