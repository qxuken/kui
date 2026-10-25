# Kit samples

Four semi-real apps on the `kui` runner with Kit and believable dummy
data: to see Kit at work in a whole window rather than a gallery, and to
start an app of your own from. Each has a manifest, a README, a headless
test that drives its `update` and checks the state, and a picture script.

| Sample | What it is | Pictures |
| --- | --- | --- |
| [relay](relay/) | An HTTP client that serves its own mock API (Larder, a grocery-delivery backend): history, method and URL, params and headers, body, and the response's status, body, headers and timing waterfall. Lua. | [body](relay/shots/relay-body.png) · [timing](relay/shots/relay-timing.png) · [headers](relay/shots/relay-headers.png) · [POST](relay/shots/relay-post.png) · [not JSON](relay/shots/relay-badjson.png) · [error](relay/shots/relay-error.png) · [sending](relay/shots/relay-sending.png) · [empty](relay/shots/relay-empty.png) |
| [watchtower](watchtower/) | An ops dashboard over a simulated fleet mid-incident: headline stats, live charts, a sortable services table, a service's detail with restart and scale, the event log. **Teal** (`kui check`). | [overview](watchtower/shots/watchtower-overview.png) · [attention](watchtower/shots/watchtower-attention.png) · [restart](watchtower/shots/watchtower-restart.png) · [scale](watchtower/shots/watchtower-scale.png) · [errors](watchtower/shots/watchtower-errors.png) · [spread](watchtower/shots/watchtower-spread.png) · [empty](watchtower/shots/watchtower-empty.png) |
| [field-notes](field-notes/) | Notes with checklists in notebooks, kept in sqlite: search, pin, sort, autosave, a confirm before deleting. Lua. | [note](field-notes/shots/field-notes-note.png) · [checklist](field-notes/shots/field-notes-checklist.png) · [search](field-notes/shots/field-notes-search.png) · [delete](field-notes/shots/field-notes-delete.png) · [empty](field-notes/shots/field-notes-empty.png) · [error](field-notes/shots/field-notes-error.png) |
| [preferences](preferences/) | A settings window (Tidewater, a pretend sync client): sections, every input kind, validation, search across settings, save and revert over TOML. Lua. | [general](preferences/shots/preferences-general.png) · [sync](preferences/shots/preferences-sync.png) · [network](preferences/shots/preferences-network.png) · [invalid](preferences/shots/preferences-invalid.png) · [a bad name](preferences/shots/preferences-named.png) · [notifications](preferences/shots/preferences-notifications.png) · [advanced](preferences/shots/preferences-advanced.png) · [search](preferences/shots/preferences-search.png) · [nothing found](preferences/shots/preferences-nothing.png) · [reverted, with Undo](preferences/shots/preferences-reverted.png) · [reset](preferences/shots/preferences-reset.png) |

Every picture names its skin in `test/shots.nuon`: each sample is shown
in Ledger, Neutral and Phosphor, in the dark appearance unless the entry
says `appearance: "light"`.

```bash
kui run addons/kit/samples/relay                       # any of the four
cd addons/kit/samples/relay && kui run test --headless  # its test; the exit code is the failures
nu addons/kit/samples/shots.nu                          # every sample's shots/ again (or name some)
cargo test -p kui-cli --test run sample                 # all four tests, and Watchtower's types
```

Every sample is Kit as it is, with no stand-ins: the confirm dialogs
are `kit.dialog`, the note and the request body `kit.text_area`, a new
note's title `autofocus`, Preferences' fields carry their own `error` and
its toast an Undo. A sample that has to draw something Kit should have
drawn is a Kit bug to file.
