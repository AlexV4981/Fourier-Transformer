# Fourier Transformer — Rust port

A native desktop rewrite of transform.py. The original Python implementation remains available for comparison.

## Run (Windows / Linux)
Install stable Rust from https://rustup.rs/ and on Windows install Visual Studio Build Tools with Desktop development with C++. On Linux install the native windowing/graphics dependencies required by eframe and rfd.

```sh
cargo run --release
cargo test
```

Select text or PNG/JPEG/BMP, set detail, calculate, then export Desmos HTML and open the saved file in a browser. The HTML requires internet access and the original project's Desmos API demo key; no image is uploaded by the Rust application. Manual equations and clipboard output are available.

## Memory design
- 4 megapixel input ceiling, 64 MiB decoder allocation limit (decoder-dependent).
- Maximum 200 paths, 500 requested coefficients per path, 12,000 retained coefficients total.
- Compact f32 coefficients (12 bytes each, at most 144,000 bytes of coefficient payload); previews independently decimated.
- FFT per contour; no retained input image or FFT buffers in completed drawings.
- One worker at a time; old results dropped before processing.
- On-demand manual equations; HTML streamed one expression at a time. UI displays only the first 4,000 characters of an equation.
- Browser memory is separate: Desmos may still consume substantial RAM when graphing thousands of terms.

These limits bound major data structures, not total process RSS. Contour extraction still temporarily stores all detected boundaries; pathological images can use substantial RAM. Resize large images before use. Text uses installed fonts and separate closed glyph outlines rather than joining unrelated contours; image threshold and tracing are approximate equivalents, not bit-identical OpenCV output. HTML uses per-path restricted pen points instead of one giant piecewise point. Manual export retains the master pen. Browser auto-launch replaced with explicit file export.

## Validation
Unit tests cover FFT frequency, repeated points and export structure. Windows/Linux CI checks the build and runs tests. No measured RAM reduction or visual parity is claimed until tested on identical inputs. Compare process peak working set in Task Manager or /usr/bin/time -v using the same image, contour count and detail. Measure browser separately and check repeated calculations, large images, symbols, and Desmos animation.

## Browser edition (GitHub Pages)

The browser version uses the same Rust FFT, contour and Desmos engine through WebAssembly in a dedicated Web Worker. Text is rasterized locally with browser fonts (not identical to native glyph outlines); images use Rust decoding and thresholding. Play, pause, reset, speed, source outlines, epicycle circles, manual equations and HTML export are supported. Input stays in the browser. The canvas uses 256 reconstruction steps per path for its visual animation; Desmos export uses actual retained coefficients.

For GitHub Free: review commit history before making the repo public, then Settings → Pages → Source → GitHub Actions. Run the Deploy Fourier Web workflow manually after enabling Pages. Site URL: https://alexv4981.github.io/Fourier-Transformer/ (after successful deployment). No backend or hosting bill. The workflow builds the WASM artifact before publishing. Native and original Python versions remain in this repository.

Local web build (optional): install wasm-pack, run `wasm-pack build web --target web --release --out-dir site/pkg`, then serve web/site over HTTP. Do not open index.html using file://.

### Color image tracing and WebP
Both editions accept static WebP, PNG, JPEG and BMP. Color-aware mode (default) uses maximum-channel Sobel gradients and directional thinning, preserving color boundaries even at equal luminance. Transparent pixels are composited against white. Color paths carry representative RGB stroke colors through previews and Desmos HTML; this is a colored line drawing, not a filled raster reconstruction. Use classic grayscale for simple silhouettes, lower edge sensitivity for busy photographs, higher sensitivity for low-contrast artwork. Color tracing downsamples to at most 1.5 MP after the 4 MP decode check; contour and global coefficient caps still apply, with a length-weighted allocation reserving detail for later paths. Path colors are representative, not exact per-pixel colors. Unit tests include near-equal-luminance boundaries and static lossless WebP round-trip.

### Verification-gated deployment
Pages now publishes only after the native Windows/Linux tests, release WebAssembly build, and real headless Chromium browser regression pass. Synthetic fixtures exercise PNG/JPEG/BMP/WebP, transparency, color boundaries, input limits, repeated noisy high-detail calculations, exports and UI recovery. Browser tests cover PNG/JPEG/WebP and malformed/oversized input, color strokes, playback, clipboard and Desmos HTML. The browser retires its previous worker before subsequent calculations so old WASM linear-memory high-water allocations can be reclaimed by the browser. This does not guarantee a particular peak process RSS. No user-provided pictures are included in this public repository. Desmos itself is not loaded during CI; exported HTML is checked structurally. Run `cargo test --all-targets` for native verification. After building WASM, install browser tests with `npm install --prefix web/tests`, `web/tests/node_modules/.bin/playwright install chromium`, then `node web/tests/browser.mjs`.

### Release verification branch
The dedicated `verify-browser-release` branch runs all native and browser tests without publishing Pages. Its successful run may fast-forward main (never force push) and explicitly dispatch production deployment, which reruns all verification gates. A failure leaves main unchanged. Long equation previews retain parameter bounds; the clipboard receives the full equation.

### Desmos clipboard correction
The Copy expression only button emits just a single LaTeX expression (or a=0 for setup). Copy upper t bound supplies only the domain value. Preview prose is never sent to Desmos. HTML export still sets each path's domain and color automatically. Large Master Pen expressions may exceed Desmos complexity limits; prefer HTML export, which uses separate pen expressions. Output uses explicit LaTeX delimiters and signed phases. Browser tests check the separate clipboard payloads.

### Foreground-first modes
Subject outline and Subject high detail use scanline background estimation, morphological closing, and a dominant silhouette before selecting spatially distributed interior edges. This heuristic is designed for isolated subjects on simple backgrounds; it can include cast shadows or fail on busy backgrounds, where classic color-aware mode is preferable. High detail increases the cap to 400 paths and 24,000 coefficients, reduces coefficient pruning, and permits up to 2,048 FFT samples per long contour. The browser adapts reconstruction sampling to retained frequencies. This is not pixel-exact raster reconstruction. The 4 MP decode and 1.5 MP tracing ceilings remain. Desmos can be slow with hundreds of paths; use smaller settings if needed. Uploaded examples are not published. The real Desmos parser gate remains mandatory; multiplication is now expressed by juxtaposition to avoid the observed undefined cdott variable.
