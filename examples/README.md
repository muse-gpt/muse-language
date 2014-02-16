# Muse examples

The examples are source-first. Generated media belongs in a local `dist` directory and is not committed.

```powershell
cargo run -p muse-tools -- export examples/game-battle-arc.muse --format midi -o examples/dist/game-battle-arc.mid
cargo run -p muse-tools -- export examples/game-battle-arc.muse --format musicxml -o examples/dist/game-battle-arc.musicxml
cargo run -p muse-tools -- export examples/game-battle-arc.muse --format pdf -o examples/dist/game-battle-arc.pdf
cargo run -p muse-tools -- export examples/game-battle-arc.muse --format svg -o examples/dist/game-battle-arc.svg
cargo run -p muse-tools -- export examples/game-battle-arc.muse --format html -o examples/dist/game-battle-arc.html
cargo run -p muse-tools -- render examples/game-battle-arc.muse --format wav -o examples/dist/game-battle-arc.wav
cargo run -p muse-tools -- render examples/game-battle-arc.muse --format mp3 -o examples/dist/game-battle-arc.mp3
```

`game-battle-arc.muse` demonstrates a deterministic five-act battle arc. `minimal-theme.muse` is the smallest complete composition.
