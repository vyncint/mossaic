# mossaic GitHub Action

Runs the contribution-art tracker on a schedule and hands back the numbers:
how many letter days are bright, what today owes, and whether the text can
still be drawn at all. Sending that somewhere — Slack, Discord, email, an
issue — is a step you add after it.

```yaml
name: contribution art
on:
  schedule:
    - cron: "0 1 * * *" # 08:00 in UTC+7, daily
  workflow_dispatch:

jobs:
  track:
    runs-on: ubuntu-latest
    steps:
      - id: art
        uses: vyncint/mossaic/action@main
        with:
          text: VYNCINT
          year: "2027"
          start-week: "6" # keep this fixed: the plan is these inputs
          timezone: Asia/Ho_Chi_Minh
      - run: echo "${{ steps.art.outputs.headline }}"
```

No `actions/checkout` needed — the action queries the API, it does not read
your repository.

## Setting up a repository for it

The tracker needs somewhere to run, and a public repository runs Actions for
free. It does not need to be the repository holding your art commits; an empty
one is fine.

1. **Create a public repository** — `contribution-art`, say. Nothing in it but
   the workflow.
2. **Add `.github/workflows/track.yml`** — copy
   [`track.example.yml`](track.example.yml) and edit the `with:` block.
3. **Add whichever secret your channel needs** (Settings → Secrets and
   variables → Actions). The issue-comment route below needs none.
4. **Run it once by hand** — the Actions tab, *contribution art*, *Run
   workflow*. Schedules only start firing after the workflow is on the default
   branch, and a manual run tells you the setup works today rather than
   tomorrow.

Two things GitHub does that will bite you otherwise:

- **A scheduled workflow in a public repository is disabled after 60 days
  without commits.** GitHub emails you first. A repository whose only job is a
  daily cron will hit this; either push something occasionally or re-enable it
  from the Actions tab when it happens.
- **`cron` is UTC**, always. `0 1 * * *` is 08:00 in UTC+7. Set `timezone:` to
  the one your GitHub profile uses, or the day the report calls "today" will
  disagree with the day your graph shades.

## Inputs

| input | default | what it is |
| --- | --- | --- |
| `text` | *required* | what you are drawing, e.g. `VYNCINT` |
| `user` | repository owner | whose contributions to compare against |
| `year` | this year | which calendar |
| `start-week` | centred | left edge, in columns — **keep it fixed** |
| `top` | `1` | first row used, 0 = Sunday |
| `background` | `0` | draw the background at this level instead of leaving it empty — **keep it fixed** |
| `timezone` | `UTC` | what "today" means |
| `token` | `github.token` | see *Private contributions* below |
| `fail-on` | `never` | `never`, `behind`, or `holed` |
| `summary` | `true` | write the report to the job summary |

**The plan is these inputs.** Tracking with a different `start-week` — or a
different `background` — compares against a different plan and reports nonsense
confidently. The report prints the placement it used on its second line; if that
ever changes, so did your plan.

**`background`** turns the rest of the year into part of the picture rather
than something to keep dark. `background: "1"` draws the letters at level 4 on
a level-1 field, so you can draw art and still contribute every day. Leave at
least two levels between the two — the `legibility` output says `clear`,
`readable` or `faint`, measured across every palette GitHub ships. See
[docs/ART.md](../docs/ART.md#drawing-on-a-background-not-on-nothing).

## Outputs

`verdict` (`drawn` / `reachable` / `holed`), `headline` (one line, fit for a
notification title), `markdown` (the whole report, fit for a message body),
`json` (everything), and the scalars: `bright`, `letters`, `owing-days`,
`owing-commits`, `holes`, `today-short`, `tomorrow-need`.

With a `background` set, also: `field-level`, `field-days`, `field-bright`,
`field-owing-days`, `field-owing-commits`, plus `legibility` (`clear` /
`readable` / `faint`) and `separation` (the CIE76 ΔE between the two shades in
the worst palette a reader might have).

`today-short` counts a background day too, so a daily "what do I owe today"
notification keeps working unchanged when you add one.

## Sending it somewhere

Each of these goes after the `uses:` step in the same job. Inputs reach the
shell through `env:`, never interpolated into `run:` — a report is data, and
data does not belong in a command line.

**An issue comment.** No secrets, nothing to sign up for; good first choice.

```yaml
      - name: Comment on the tracking issue
        env:
          GH_TOKEN: ${{ github.token }}
          BODY: ${{ steps.art.outputs.markdown }}
        run: gh issue comment 1 --repo "$GITHUB_REPOSITORY" --body "$BODY"
```

**Slack**, through an [incoming webhook][slack]:

```yaml
      - name: Slack
        env:
          WEBHOOK: ${{ secrets.SLACK_WEBHOOK_URL }}
          TEXT: ${{ steps.art.outputs.headline }}
          BODY: ${{ steps.art.outputs.markdown }}
        run: |
          jq -n --arg t "$TEXT" --arg b "$BODY" \
            '{text: $t, blocks: [{type: "section", text: {type: "mrkdwn", text: $b}}]}' \
            | curl -sSf -X POST -H 'Content-type: application/json' -d @- "$WEBHOOK"
```

**Discord**, through a channel webhook (Server Settings → Integrations):

```yaml
      - name: Discord
        env:
          WEBHOOK: ${{ secrets.DISCORD_WEBHOOK_URL }}
          BODY: ${{ steps.art.outputs.markdown }}
        run: |
          jq -n --arg c "$BODY" '{content: $c}' \
            | curl -sSf -X POST -H 'Content-type: application/json' -d @- "$WEBHOOK"
```

**Email**, through any SMTP account (Gmail needs an [app password][app]):

```yaml
      - name: Email
        uses: dawidd6/action-send-mail@v18
        with:
          server_address: smtp.gmail.com
          server_port: 465
          secure: true
          username: ${{ secrets.MAIL_USERNAME }}
          password: ${{ secrets.MAIL_PASSWORD }}
          from: contribution art
          to: ${{ secrets.MAIL_TO }}
          subject: ${{ steps.art.outputs.headline }}
          html_body: ${{ steps.art.outputs.markdown }}
```

**Only when something needs doing**, which is the setting most people end up
wanting — a daily message that says "nothing to do" gets muted within a week:

```yaml
      - name: Slack, only when today owes something
        if: steps.art.outputs.today-short != '0'
        ...
```

`fail-on: behind` does the same thing through the job status, so a failed run
lands in the notifications you already have.

## Notes

- The tracker is built from the action's own ref on first use and cached; a
  cold run adds a minute or two, warm runs seconds.
- The action tracks whichever ref you pin — `@main`, a tag, or a SHA. Pin a tag
  or SHA if you want it to change only when you say so.
- **Private contributions.** The default `GITHUB_TOKEN` sees public
  contributions only. Your own graph counts private ones if you have
  [that setting][private] on, so a token without `read:user` will report you
  behind when you are not. Use a fine-grained PAT with read access to your
  profile and pass it as `token:`.
- The action never writes commits. It reads the API and reports; `mossaic-art --write`
  is a thing you run yourself, on purpose, locally.

[slack]: https://api.slack.com/messaging/webhooks
[app]: https://support.google.com/accounts/answer/185833
[private]: https://docs.github.com/en/account-and-profile/setting-up-and-managing-your-github-profile/managing-contribution-settings-on-your-profile/showing-your-private-contributions-and-achievements-on-your-profile
