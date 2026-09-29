# How WARP orders mods

## What the game does

Warhammer III loads every enabled pack and, when two packs contain the same
thing, one of them wins:

| Content | Who wins |
|---|---|
| Files at the same path (scripts, UI, variants, startpos, images, models) | The pack **higher** in the load order |
| DB and text tables | Decided by the table *file name* inside each pack, not the pack's position (unless two packs ship the identical table file, in which case position decides) |

Two consequences:

1. Order only matters between packs that actually collide. Most pairs of mods in
   a 130-pack list never touch the same file, and their relative order is
   irrelevant.
2. When packs *do* collide, the more specific one has to win. A submod has to beat
   the mod it changes, and a UI tweak has to beat the overhaul that also ships UI
   files.

## The rule: specific above general

WARP's default order puts specific content on top and foundations at the bottom:

```
Experimental            <- highest priority, wins collisions
Audio
UI
Battle maps
Battle
Graphics
Animations
Campaign map
Units
Campaign
Overhaul
Core (frameworks, assets, game modes)   <- lowest priority, the foundation
```

Within each tier, the **role** applies the same idea: submods and patches, then
fixes, then regular content, then frameworks, then shared assets. Pack names
break the remaining ties in Kaedrin character order, so the result is always the
same for the same inputs. That matters for multiplayer: two players with the same
mods get the same order.

This rule came from practice. It is what kept 130+ pack campaigns stable in
W.A.R.P. v1. The tier and role priorities live in
[`knowledge/taxonomy.toml`](../knowledge/taxonomy.toml).

## Hard rules on top

The tier rule is the *default*. Hard rules override it, and every placement says
which rule put it there:

- **requires**: a mod sits above everything it requires (a dependent overrides
  what it builds on).
- **patches**: a patch or submod sits above the mod it patches.
- **pins**: explicit "A above B" choices by the user.

Collisions don't move packs, but WARP shows them: the conflict map lists every
pair of packs that ship the same files, who wins, and how risky that is, and
flags mods that look like two versions of the same mod.

When a hard rule conflicts with the default, WARP moves the *dependent* up to sit
directly above what it depends on, rather than pulling the (usually large) parent
down. If rules contradict each other (A above B above A), WARP reports the cycle
and falls back to the default order for the packs involved.
