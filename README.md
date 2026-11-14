# FFTA Clan Desk

A Windows beta of **FFTARecomp** with an integrated clan editor, a new Clan Desk menu, manual save states and a browsable timed-autosave history.

**Independently maintained by [johnnieb333](https://github.com/johnnieb333).**
Forked from [Jorge262007/FFTARecomp](https://github.com/Jorge262007/FFTARecomp), built on [JRickey/gba-recomp](https://github.com/JRickey/gba-recomp). Original authors, licences and third-party notices are retained.

## Windows beta scope

The intended first release is **v0.1.0-beta** for **Windows x86_64**. The editor and new UI have been tested with the European FFTA ROM. The beta launcher verifies that ROM before starting. Other platforms and regions are not claimed as tested for these additions.

No ROM, BIOS, save, extracted game artwork or generated game translation is included in release packages. The player supplies the ROM. Character sprites are decoded from it at runtime and cached in memory.

## Additions in this fork

- Clan Desk pause menu and roster, with actual character/job sprites and readable controls.
- Base-stat and level editing, staged previews, confirmation and full-state Undo Last Edit.
- Visible Game-possible / Unrestricted stat modes. Unrestricted has x1/x2/x5 adjustment steps; mouse-holding repeats steadily.
- Job changes that unequip all five items while preserving inventory counts; Undo restores the prior state.
- Individual ability AP, selected-job mastery and staged mastery of all jobs for a character's race.
- Clan Funds editor with clear current/new totals, visible increments and confirmation.
- Inventory search and filters, individual quantities, batch edits and Add All Unowned with quantities 1-10.
- 30 independent manual save-state slots, screenshots and recovery controls.
- 10 rotating timed autosaves, screenshot browser, load confirmation and Undo Load.
- Autosave timer: Off, 1, 2, 5 or 10 minutes, default 5. It counts active play and waits for a recognized gameplay scene before writing.
- Existing QuickSave remains separate and accessible through the browser.
- Reward multipliers, shop-stock controls, law-penalty option, renaming and character cloning.
- Targeted dialogue, law-panel and mission-result alignment fixes.

## Run the beta

Extract the complete Windows archive into a writable folder. Open **FFTA-Clan-Desk.exe** and choose your European FFTA ROM if prompted. Keep recomp.exe, gamedb.sqlite and the tcc folder beside the launcher. Players do not need Python, Rust or GCC when using the complete packaged build.

Esc opens Clan Desk. Clan & Editor contains the editing tools; Saves contains Manual and Autosaves tabs. Character and inventory editing requires the world map. Equip items and assign learned abilities through the game's own menus.

| Key | Action |
| --- | --- |
| Esc | Open/close Clan Desk |
| F5 | Save selected manual slot; confirm overwriting |
| F6 / F7 | Next / previous manual slot |
| F10 | Load selected manual slot |
| F4 | Undo the last state load |

Editor changes are staged until Apply and confirmation. Right-click is ignored in paused menus; use Esc or Back to return. Some in-game stat/funds displays refresh after changing screens.

## Save compatibility and limits

Existing FFTARecomp data-directory and save names are retained for compatibility. The launcher remembers a ROM path under Local AppData/FFTA-Clan-Desk; it does not copy the ROM. Existing save-path selection remains in effect. Timed history is stored under AutoStates in the active saves directory, separate from ManualStates and QuickSave.

Manual states are tied to ROM and BIOS identity. Cartridge saves and full-machine states are different formats; a general-purpose direct state-to-cartridge converter is not included.

Game-possible limits are a **per-stat recruitment plus level-growth envelope**, not proof that every displayed maximum can be achieved together. They exclude boosts and repeated level-down cycles. Estimated level reductions cannot reconstruct unrecorded job history or random rolls.

This is a beta. Test edited progress using separate saves. The release checklist records outstanding clean-package and gameplay checks: [docs/CLAN-DESK-RELEASE-CHECKLIST.md](docs/CLAN-DESK-RELEASE-CHECKLIST.md).

## Build

See [BUILDING.md](BUILDING.md) for the inherited toolchain requirements. The Windows development build uses:

```bash
cargo build --release -p recomp -p ffta-play -j 2
python TinyCC/build.py --cc gcc
```

After committing the reviewed source, create a Windows package with:

```bash
python scripts/package_clan_desk.py
```

The packaging script includes only listed runtime files, licence notices and the matching TinyCC source. It excludes local ROMs, saves, research reports, dumps and generated game libraries.

## Credits and licences

- **Jorge.R / Jorge262007 and contributors:** upstream FFTARecomp.
- **JRickey and contributors:** gba-recomp foundation and function metadata.
- **johnnieb333:** FFTA Clan Desk fork and its development/testing, with AI assistance.
- See [Licenses/CREDITS.md](Licenses/CREDITS.md), [Licenses/LICENSE](Licenses/LICENSE), and [Licenses/THIRD_PARTY_NOTICES.md](Licenses/THIRD_PARTY_NOTICES.md) for retained notices and component licences. TinyCC source and replacement instructions are included separately.

Final Fantasy Tactics Advance and related names belong to their respective rightsholders. This fan project is not affiliated with Square Enix or Nintendo. The software licences do not grant rights to the game.
