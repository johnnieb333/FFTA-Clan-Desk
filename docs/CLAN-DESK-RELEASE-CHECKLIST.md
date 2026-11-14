# FFTA Clan Desk v0.1.0-beta release checks

## Confirmed in the Windows development build

- Clan Desk menu readability and character sprites.
- Job change, immediate sprite refresh, safe equipment removal and Undo.
- Stat edits in Game-possible and Unrestricted modes; Speed shown in-game.
- Level reduction, keeping current stats, preview and Undo.
- Hold-to-repeat, x1/x2/x5 steps, paused-menu right-click protection.
- Clan Funds Apply and Undo; in-game totals refresh after a screen change.
- Timed autosave creation, screenshot browser, load and Undo Load.
- Autosave timer persistence after restarting and ten-slot wraparound.
- Automated menu/save tests and a successful release-profile build.

## Required before publishing the Windows beta

- [ ] Check Master all race jobs in-game after Apply (native batch tests passed).
- [ ] Save and reload manual slot 30 in-game (automated test passed).
- [ ] Build the new launcher and bundled TinyCC from the matching source.
- [ ] Test the unpacked release in a fresh folder with no translation cache or developer compiler in PATH.
- [ ] Confirm ROM picker, first translation, keyboard/controller menu access, save/reload and a second launch.
- [ ] Inspect source staging: only maintained code, docs and original licences; no personal/test/research data.
- [ ] Package and inspect the exact release ZIP, licences, TinyCC source and SHA256SUMS.
- [ ] Publish from the reviewed commit/tag; use beta wording and retain upstream credits.

Clan Talents, character presets and character export/import are deferred. Linux, Steam Deck and Android verification for this fork is not included in the Windows beta claim.
