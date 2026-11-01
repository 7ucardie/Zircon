# Original sprite authoring from the installed collection

Status: the first authoring workflow and Jade Beetle demonstration are implemented.
See [SPRITE_AUTHORING.md](SPRITE_AUTHORING.md) for the delivered behavior and
limitations. The sections below retain the design and later-work scope.

The installed assets provide animation layouts, frame timing, viewing directions,
foot anchors, shadows, overlays, palette and scale references. Those are useful
constraints for making new art. Recolouring or resizing an existing monster is
already supported by `sprite_variant.rs`; genuinely new silhouettes and motion
need an authoring workflow and visual review across the entire animation.

## Deliverable and first scope

Start with a new static item icon and one small eight-direction monster using an
existing animation contract. Keep the original libraries read-only. Export each
new asset as a separate versioned pack, with an import manifest mapping new
ItemInfo/MonsterInfo records to the generated library and frame slots. Do not
change the renderer to accommodate an inconsistent asset.

1. **Reference and animation inventory.** Export a contact sheet and JSON contract
   for a selected library: every action, direction, frame index/duration, empty
   slot, image/shadow/overlay dimensions, foot anchor and offsets. Resolve frame
   layouts from the same action tables the client uses. Record source library
   checksums so later builds cannot silently use a different pack.
2. **Editable authoring package.** Export lossless PNG layers and a manifest into
   a new folder. Keep absent slots distinct from transparent frames. Support
   authored PNG imports and a sprite-sheet importer with explicit cell dimensions
   and action/direction mapping. Preserve scale and ground-contact guides.
3. **New artwork.** Allow manual pixel-art drawing, part composition from selected
   reference frames, and optional AI-generated concepts/frames. The AI receives
   the animation contract and contact sheets; it produces editable PNGs, never
   writes raw library indices or live game records. Start with a static icon,
   then a single facing idle/walk cycle before expanding to all required actions.
4. **Consistency checks and editor preview.** Reject missing required directions,
   duplicate/conflicting slots, oversized surfaces, incompatible transparency,
   out-of-range offsets and missing shadow/overlay declarations. Preview actions
   in a loop with a ground grid, foot-anchor marker, original-reference toggle
   and contact sheets. Check flicker, body-size drift, foot sliding and shadows
   visually; dimensional validation alone cannot establish motion quality.
5. **Pack builder.** Feed validated frames through the existing ZlBuilder codec.
   Roundtrip-decode each written slot, compare alpha/shape within compression
   tolerance, preserve slot numbering and explicit blanks, and export to a new
   pack. Neutral export of an untouched source must preserve its raw slots.
6. **Game integration.** Allocate an unused appearance mapping supported by the
   client asset resolver and create draft ItemInfo/MonsterInfo records in a new
   database snapshot. Show all actions in the editor, then spawn the monster in
   an isolated Test Walk bundle. Keep balance and drops separate from artwork.

## MCP extension

Expose `sprite_reference`, `sprite_export_project`, `sprite_validate_project`,
`sprite_build_pack`, and `sprite_preview_sheet` after the underlying authoring
operations exist and are tested. Inputs and output PNG paths stay within the MCP
project; installed libraries are only reference inputs. Return structured frame
errors so an AI can repair one action/direction instead of regenerating an entire
library. New packs and database revisions are explicit outputs; publication or
replacement of installed assets remains a separate user action.

## Completion criteria

A new item icon and a recognizably new monster can be authored/imported, validated,
previewed, packaged, linked to new database records and seen in the running client.
All required idle, movement, attack, hit and death slots for the chosen contract
work in every required direction; empty/unused slots remain valid. Rebuilds are
repeatable from the authoring folder. Existing assets and render quality remain
unchanged. AI-assisted motion generation is optional; it must pass the same
contract and visual checks as hand-authored frames.

Later work: multiple body-part layers and equipment variants, additional action
contracts, palette tools, batch pack generation, animated effects and shared
project manifests. Arbitrary new animation rigs are outside this initial scope.
