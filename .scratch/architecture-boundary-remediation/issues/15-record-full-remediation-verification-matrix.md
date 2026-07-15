# 15 — Record the full remediation verification matrix

**What to build:** Demonstrate and record that the completed architecture remediation preserves every translation entry point, stored-data workflow, popup path, rapid-request case, and supported OCR profile after all review findings are fixed.

**Blocked by:** 09 — Restore local dictionary failure semantics; 10 — Make screenshot requests atomic and latest-wins; 11 — Restore explicit OCR compatibility profiles; 12 — Surface popup-ready failures; 13 — Compare translation results without allocation; 14 — Clarify the shared HTTP client accessor.

**Status:** completed

- [x] Final frontend and backend automated suites, type checks, production build, and browser acceptance test pass.
- [x] Manual input and clipboard translation use current backend settings and produce compatible persisted history records.
- [x] Selected-text translation covers UI Automation, clipboard fallback and restoration, immediate local results, later enrichment, provider failure, and rapid consecutive requests.
- [x] Screenshot translation covers selection, cancellation, OCR progress, early recognized-text backfill, provider failure, persistence, anchoring, settings changes during OCR, and out-of-order completions.
- [x] History loading, detail lookup, deduplication, access counts, favorite toggling, and existing-row compatibility are verified.
- [x] Settings, provider selection, credentials, shortcuts, and tray behavior restore correctly after restart.
- [x] Native, lite, and full OCR configurations resolve their intended engine and resources and complete their required startup or recognition smoke path.
- [x] Exact commands, profile configuration, manual steps, observed outcomes, and environmental limitations are recorded below.

## Automated verification

| Area | Command | Observed outcome |
|---|---|---|
| Final repository gate | `npm run verify:final` | Passed: 14 frontend files / 149 tests; 75 Rust library tests, 4 local-dictionary integration tests, 15 popup contract tests; `cargo check`; `vue-tsc --noEmit`; production Vite build. |
| Browser acceptance | `npm run test:e2e` | Passed: 1 Playwright golden-path test for manual translation and result rendering. |
| Python sidecar syntax | `python -m py_compile scripts/paddle_ocr_server.py` | Passed. |
| Native package | `CI=false npm run tauri:build:ocr:native` | Passed after supplying the boolean `CI=false`; produced MSI and NSIS bundles with packaged `small` models and `onnxruntime.dll`. |
| Sidecar executable | `npm run ocr:sidecar:win` | Passed; produced `src-tauri/binaries/paddle-ocr-server-x86_64-pc-windows-msvc.exe` containing PaddleOCR and RapidOCR. |
| Lite package | `CI=false npx tauri build --config src-tauri/tauri.ocr-lite.conf.json` | Passed; produced MSI and NSIS bundles with sidecar, server script, and packaged `small` models. |
| Full package | `CI=false npx tauri build --config src-tauri/tauri.ocr-sidecar.conf.json` | Passed; produced MSI and NSIS bundles with the sidecar and explicit official-download profile. |

`CI=1` from the harness is not a valid Tauri CLI boolean and caused the first native build attempt to stop before compilation. Re-running with `CI=false` completed normally; this is an invocation environment constraint, not a product failure.

## Translation and persistence matrix

| Contract | Evidence and observed outcome |
|---|---|
| Manual input | Playwright golden path plus `TranslatePage.mount.spec.ts` and `translation.spec.ts`: trimmed text reaches `translate_text`, and the backend-persisted record becomes the visible result and history source. |
| Manual clipboard | `TranslatePage.mount.spec.ts` triggers the clipboard action; `translation.spec.ts` verifies `translate_from_clipboard` owns acquisition and persistence and returns the persisted record. |
| Current settings per request | `workflow_reads_provider_settings_for_each_request` passed; changing provider configuration affects the next request without frontend credential snapshots. |
| Local hit, miss, and failure | `local_result_is_reported_before_enrichment`, `local_miss_uses_current_configured_provider`, and `local_lookup_error_stops_before_online_collaborators` passed. Local lookup errors call neither enrichment nor providers. |
| Selected text | `ui_automation_result_is_preferred_without_touching_clipboard`, `empty_ui_automation_result_uses_clipboard_fallback`, and `ui_automation_failure_uses_clipboard_fallback` passed. |
| Clipboard restoration | `clipboard_fallback_restores_previous_text_after_success` and `clipboard_fallback_restores_previous_text_after_read_failure` passed; restoration occurs on both result paths. |
| Staged translation | `local_enrichment_persists_once_and_updates_without_incrementing`, `provider_failure_keeps_dictionary_context`, and `stale_request_cannot_publish_enrichment_or_completion` passed. |
| Screenshot atomicity | Frontend tests passed for provider failure after OCR backfill, cancellation, and two requests completing out of order. Backend tests passed for one settings snapshot, OCR backfill before completion, empty OCR rejection, provider failure, persistence, and stale-stage suppression. |
| Popup readiness | `popup-window-runtime.spec.ts` and `popup-window-chrome.spec.ts` passed: five listeners register before `popup-ready`; a rejected ready event propagates and renders a terminal error without leaving loading active. |
| History and detail | Database and store tests passed for history, favorites, detail lookup, list-field round trips, existing-row updates, and access-count behavior. |
| Deduplication | `equal_local_and_completed_results_do_not_update_persistence` passed; changed enrichment persists once. Borrowed content comparison tests passed for equal content, changed content, and differing persistence metadata. |
| Settings and restart persistence | Rust and frontend save/load round-trip tests passed for providers, credentials, OCR engine/profile, shortcuts, theme, tray, and zero-value compatibility. |
| Shared HTTP client | All six OCR/provider call sites use `http_client::shared_client()`; the old accessor has no remaining call sites or compatibility alias. Focused OCR tests and backend compilation passed. |

## OCR profile and runtime matrix

| Profile | Exact smoke configuration | Observed outcome |
|---|---|---|
| Native | Launch `src-tauri/target/release/translation-tool.exe` from the native build with startup preload enabled. | Application log reported ONNX Runtime `1.20.1` loaded and `原生 OCR 初始化完成: profile=small` in 143 ms using the packaged `small` model directory. |
| Lite Paddle local | `paddle-ocr-server-x86_64-pc-windows-msvc.exe --engine paddleocr --model-profile small --model-dir src-tauri/resources/ocr-models/small --port 8873` | `/health` returned engine `paddleocr`, profile `small`, source `local`; `/ocr` returned HTTP 200 and recognized text from `docs/assets/product-preview.png`. |
| Full Paddle official | `paddle-ocr-server-x86_64-pc-windows-msvc.exe --engine paddleocr --model-profile official --allow-official-model-download --port 8871` | `/health` returned engine `paddleocr`, profile `official`, source `official-download`; official models initialized and `/ocr` returned HTTP 200 with recognized text. |
| RapidOCR | Packaged: `paddle-ocr-server-x86_64-pc-windows-msvc.exe --engine rapidocr --model-profile embedded --port 8870`; final shared-runtime check: `uv run --python 3.11 --with rapidocr-onnxruntime==1.4.4 --with onnxruntime==1.27.0 --with "numpy<2" python scripts/paddle_ocr_server.py --engine rapidocr --port 8874`. | Both `/health` responses returned engine `rapidocr`, profile/source `embedded`; both `/ocr` requests returned HTTP 200 with recognized text. The packaged Paddle sidecar did not change the selected RapidOCR engine, and the shared ONNX Runtime `1.27.0` pin completed recognition. |
| Missing local resources | Start PaddleOCR with `--model-profile small` and no `--model-dir`. | Process exited before server startup with `PaddleOCR small 配置缺少本地模型目录。请传入 --model-dir。`; recognition was never entered. |

The smoke image was `docs/assets/product-preview.png`. Each successful sidecar run returned the selected engine and model source from `/health`, then recognized visible Chinese and English UI text through `/ocr`.

## Browser/UI steps

1. Started Vite on `http://127.0.0.1:4173` and opened `/settings` in Chromium.
2. Confirmed the native default renders `原生 ONNX`, `Small（本地打包模型）`, the in-process runtime source, ONNX/PP-OCR version text, and preload controls without clipping or overlap.
3. Selected RapidOCR and observed the model profile switch to `Embedded（RapidOCR 内置模型）`, the Sidecar address appear, and RapidOCR/ONNX Runtime versions update.
4. Selected PaddleOCR and confirmed incompatible `embedded` state is replaced with the packaged `small` profile; the model selector exposes local `tiny`/`small`/`medium` and explicit `official` download choices.

The browser-only settings session had no Tauri IPC bridge, so saving produced the expected `invoke`-unavailable toast. Persistence itself is covered by the Rust/frontend round-trip tests and the packaged application startup smoke.

## Environmental limitations

- No production Youdao or Microsoft credentials were used. Provider success, fallback, missing-credential, and failure behavior were exercised through deterministic workflow collaborators rather than billed external calls.
- Global shortcuts, tray clicks, UI Automation against a third-party desktop editor, and interactive screenshot rectangle selection were not replayed as unattended OS input in the final pass. Their orchestration, fallback, restoration, cancellation, anchoring, and stale-request behavior are covered by focused Rust/frontend tests.
- Paddle official model download required network access and succeeded during both direct and packaged sidecar smoke runs.
- All spawned Vite, Tauri, PaddleOCR, and RapidOCR smoke processes were stopped after verification.
