# Examples

Runnable `.muse` demos. Write generated media to a local folder; do not commit outputs.

| File | Shows |
| --- | --- |
| [`minimal-theme.muse`](./minimal-theme.muse) | Smallest complete composition |
| [`game-battle-arc.muse`](./game-battle-arc.muse) | Five-act arc + `orchestrate` |
| [`accidentals.muse`](./accidentals.muse) | Accidentals in a motif |

```bash
muse check examples/minimal-theme.muse
muse export examples/minimal-theme.muse --format midi -o out.mid
muse preview examples/minimal-theme.muse
```
