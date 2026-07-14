# Translation Architecture Boundary Remediation

Status: ready-for-agent

## Problem Statement

The application works, but several core workflows no longer have a single architectural owner.

From the user's perspective, manual translation, selected-text translation, and screenshot translation are the same capability entered through different surfaces. Internally, however, those surfaces obtain settings differently and do not all execute the same translation policy. A settings cache in the frontend is passed back into some backend commands, while shortcut-driven translation reads backend runtime state directly. This creates a risk that two user actions perform the same translation with different provider configuration.

The OCR subsystem also contains circular dependencies between its facade, service lifecycle, and native engine implementation. Translation results are represented by a type owned by the database module, causing persistence fields and SQLite-oriented data shapes to leak into translation orchestration, popup behavior, IPC responses, and frontend state.

Selected-text translation duplicates part of the local-dictionary and Free Dictionary resolution policy instead of delegating to the same workflow used by manual and screenshot translation. The resulting shortcut orchestrator has a high dependency fan-out and is difficult to test without real Tauri, operating-system, network, OCR, and clipboard infrastructure.

The user needs these boundaries corrected without changing existing translation behavior, stored data, shortcuts, OCR packaging options, popup behavior, or the supported translation providers.

## Solution

Create one backend-owned translation workflow that serves manual input, clipboard input, selected-text shortcuts, and screenshot OCR translation. The workflow will read the persisted backend runtime configuration, apply one translation resolution policy, and report staged progress for surfaces that need immediate local results followed by enrichment.

Separate the core translation result from its persisted record. Translation and OCR workflows will operate on domain-level results; the persistence adapter will map those results to records containing identifiers, access counts, and favorite state. The existing database schema and externally observable record format will remain compatible.

Make the OCR dependency graph acyclic. Neutral OCR configuration and status types will be independent of engine implementations. A single OCR facade will select and coordinate native or compatibility-sidecar adapters; adapters will not import the facade or lifecycle owner.

Introduce one primary testing seam at the backend translation workflow. Narrow dictionary, translation-provider, and OCR collaborators will be injected there. Existing in-memory SQLite and Vue/Vitest boundaries will continue to cover persistence mapping and user-visible frontend behavior.

Remove duplicate no-op popup event listeners so translation events have one frontend owner. Complete the change as a clean cutover: all callers migrate to the unified workflow, with no deprecated command variants, compatibility wrappers, or parallel business policies left behind.

## User Stories

1. As a user translating manually entered text, I want the backend to use my persisted provider settings, so that the result matches the settings shown in the application.
2. As a user translating selected text with a shortcut, I want the same provider and fallback policy as manual translation, so that entry method does not change the result.
3. As a user translating a screenshot, I want OCR output to enter the same translation workflow as typed text, so that screenshot translation follows identical dictionary and provider rules.
4. As a user changing translation providers, I want the change to affect every translation entry point immediately, so that no window or shortcut uses stale configuration.
5. As a user updating API credentials, I want credentials to be read from the backend source of truth, so that each translation request does not depend on a separately synchronized frontend copy.
6. As a user of the local dictionary, I want single-word translations to continue preferring local data, so that common lookups remain fast and available offline.
7. As a user receiving a local dictionary result, I want Free Dictionary enrichment to continue adding phonetics, examples, and synonyms when available, so that fast results can still become complete results.
8. As a user viewing a shortcut popup, I want the local result to appear without waiting for remote enrichment, so that the popup remains responsive.
9. As a user viewing a shortcut popup, I want a later enrichment result to update the same active request, so that I receive improved content without duplicate windows.
10. As a user issuing shortcuts rapidly, I want stale results to be ignored, so that an older request cannot overwrite the newest popup.
11. As a user translating a sentence, I want it to use the configured online provider, so that existing Youdao and Microsoft Translator behavior is preserved.
12. As a user whose local dictionary misses a word, I want the workflow to try Free Dictionary and then the configured provider, so that the current fallback behavior remains available.
13. As a user whose configured provider fails, I want a clear error that retains the relevant dictionary or provider context, so that I can diagnose the failure.
14. As a user performing screenshot OCR, I want OCR progress and translation progress to remain distinguishable, so that the popup communicates what the application is doing.
15. As a user of native ONNX OCR, I want recognition, warmup, status, and model discovery to continue working, so that the recommended packaged runtime is unchanged.
16. As a user of a compatibility OCR build, I want PaddleOCR or RapidOCR sidecar behavior to remain available, so that compatibility packages are not broken by the refactor.
17. As a user cancelling screenshot selection, I want the popup workflow to stop cleanly, so that no empty or stale result appears.
18. As a user whose OCR result is empty, I want the workflow to fail explicitly without calling translation providers, so that unnecessary requests are avoided.
19. As a user using Windows UI Automation selection reading, I want it to remain the preferred selected-text source, so that the clipboard is not modified when direct selection reading works.
20. As a user whose target application does not support UI Automation, I want the clipboard fallback and clipboard restoration behavior preserved, so that selected-text translation still works broadly.
21. As a user viewing translation history, I want existing records to remain readable after the refactor, so that no local data is lost.
22. As a user saving a translation, I want deduplication and access-count behavior preserved, so that history ordering and frequency information remain correct.
23. As a user toggling favorites, I want favorite state to remain stable across reloads, so that architectural cleanup does not change saved content.
24. As a user opening a translation detail, I want the same record fields and content as before, so that the existing frontend remains compatible during the internal model split.
25. As a user opening the popup, I want theme updates, close behavior, dragging, and Escape handling to remain unchanged, so that cleanup of event ownership is invisible to me.
26. As a user opening the main window from the popup, I want the same navigation and close behavior, so that popup control simplification does not regress the workflow.
27. As a user restarting the application, I want persisted settings and shortcuts to restore before translations can run, so that startup behavior is deterministic.
28. As a privacy-conscious user, I want credentials to remain in backend-managed settings instead of being repeated in every translation command payload, so that sensitive configuration has a smaller observable surface.
29. As a maintainer, I want one translation workflow to own dictionary priority, enrichment, provider selection, and fallback, so that policy changes have one implementation point.
30. As a maintainer, I want selected-text and screenshot handlers to focus on input acquisition and presentation, so that they do not also own translation business rules.
31. As a maintainer, I want OCR modules to depend in one direction, so that an OCR engine can be changed without editing its caller and lifecycle owner in a cycle.
32. As a maintainer, I want translation domain objects independent of SQLite records, so that translation logic does not know about record identifiers, access counts, or favorite storage encoding.
33. As a maintainer, I want persistence mapping to be explicit at the database boundary, so that schema changes do not leak into translation providers or popup orchestration.
34. As a maintainer, I want one backend runtime settings source, so that adding a provider option does not require threading another primitive through every IPC call.
35. As a maintainer, I want narrow workflow collaborators rather than a dependency-injection framework, so that testability improves without adding a second architecture problem.
36. As a maintainer, I want popup domain events to have one frontend owner, so that listener registration and cleanup are predictable.
37. As a tester, I want to substitute dictionary, provider, and OCR behavior at one workflow seam, so that success, fallback, and failure paths are deterministic.
38. As a tester, I want to observe workflow progress stages as public behavior, so that progressive popup results can be verified without inspecting implementation details.
39. As a tester, I want persistence tests to use an isolated in-memory database, so that mapping, deduplication, favorites, and access counts are verified without user data.
40. As a tester, I want frontend tests to observe command requests and rendered popup behavior, so that refactoring internal modules does not make tests brittle.
41. As a future contributor adding a translation provider, I want to implement one provider adapter and register it once, so that the change does not modify every UI entry point.
42. As a future contributor adding an OCR engine, I want to implement an adapter against neutral OCR contracts, so that engine code never imports its orchestrator.
43. As a new developer, I want module names and dependency direction to reveal ownership, so that I can identify where translation, OCR, settings, and persistence changes belong.
44. As a release engineer, I want native, lite, and full OCR build profiles to remain buildable, so that the refactor does not remove documented compatibility packages.
45. As a release engineer, I want existing serialized settings and translation records to remain compatible, so that upgrades do not require a user-data migration.
46. As a solo developer, I want the highest-risk workflows covered at one application seam, so that future changes can be made without maintaining a large mock graph.

## Implementation Decisions

- A new backend application workflow will become the single owner of translation resolution. Manual text, clipboard text, selected-text shortcuts, and screenshot OCR entry points will all delegate to it.
- The workflow will accept the text to translate plus injected collaborators. It will not accept API credentials, provider names, OCR endpoints, or persistence fields from frontend callers.
- Backend runtime configuration will be the only source for provider credentials, provider selection, OCR selection, model profile, and related translation settings. Frontend state remains a reactive presentation cache, not an alternate runtime authority.
- Translation commands will be simplified so their public request payloads contain only user input and interaction-specific data. Every frontend caller will migrate in the same cutover.
- The workflow will expose staged progress suitable for shortcut and screenshot presentation. Required stages include input accepted, local result available, enrichment available, remote translation in progress, OCR in progress where applicable, completed, cancelled, and failed.
- Progress reporting is part of the application behavior, not a Tauri-specific concern. The Tauri and popup layers translate workflow progress into existing window events and messages.
- Local dictionary priority, Free Dictionary enrichment, configured-provider fallback, and existing error precedence will have one implementation in the workflow.
- The selected-text shortcut handler will retain responsibility for shortcut registration, selected-text acquisition, request identity, and popup presentation. It will no longer call dictionary or provider adapters directly.
- The screenshot handler will retain responsibility for region selection and result anchoring. OCR and subsequent translation will be coordinated through the unified workflow.
- A core translation result will represent translated content and language information without database identity, access count, or favorite state.
- A persisted translation record will represent the database and IPC record, including identity, access count, and favorite state. The database adapter will own mapping between core results, rows, and records.
- The existing SQLite schema, unique translation key, history ordering, access-count behavior, and favorite semantics will not change.
- The existing serialized translation record remains backward compatible for the frontend during this phase. Internal domain separation must not require a user-data migration.
- SQLite integer storage for favorite state may remain an adapter detail; domain and presentation behavior should use boolean meaning rather than propagate storage encoding into core logic.
- Neutral OCR configuration, status, and engine contracts will be moved outside the facade and engine implementations.
- The OCR facade will coordinate engine selection and common operations. Native OCR and compatibility-sidecar OCR will be leaf adapters that depend only on neutral contracts and platform/vendor libraries.
- Sidecar process lifecycle, health checks, command construction, and logging will remain inside the sidecar adapter boundary. Native OCR will not depend on sidecar lifecycle types.
- The documented native ONNX runtime remains the default. PaddleOCR and RapidOCR debug and compatibility build paths remain supported.
- The primary new seam will be the translation workflow. It will receive narrow dictionary, translation-provider, and OCR collaborators; no general service container or dependency-injection framework will be introduced.
- Existing application setup remains the composition root and wires concrete adapters into the workflow.
- Static or global vendor resources may remain inside leaf adapters when they are expensive and process-wide, but core workflow tests must not depend on those globals.
- Popup window controls will own only Escape handling, close, drag, cleanup, and popup-ready signaling. Theme and translation events will remain owned by the popup component.
- Duplicate no-op event subscriptions will be removed rather than retained as compatibility hooks.
- Existing settings serialization remains compatible. This work does not replace the settings file or introduce a new settings database.
- Existing user-visible errors should remain stable unless a message must change to accurately identify the failing workflow stage.
- No deprecated command variants, aliases, parallel workflows, or temporary dual paths will remain after migration.

## Testing Decisions

- Tests will verify externally observable behavior: returned translation results, progress stages, selected provider behavior, persistence outcomes, emitted popup updates, and rendered user state. Tests will not assert source layout, private helper calls, or exact internal dependency wiring.
- The primary test seam is the backend translation workflow. Tests will inject deterministic dictionary, provider, and OCR collaborators.
- Workflow tests will cover local dictionary hits, Free Dictionary enrichment, local misses, provider fallback, provider failure, empty input, empty OCR text, and stage ordering.
- Workflow tests will verify that manual, clipboard, selected-text, and screenshot entry points reach the same translation policy with the same backend settings.
- Workflow tests will verify that a newer request prevents an older staged result from replacing current popup state where request identity is relevant.
- OCR facade tests will verify observable adapter selection, warmup, health, restart, and recognition behavior without asserting internal module calls.
- Native and sidecar vendor integrations will retain focused adapter tests where local deterministic behavior exists. Real model, process, and network execution belongs to smoke or integration verification rather than ordinary unit tests.
- Persistence mapping will reuse the existing in-memory SQLite testing style. Tests will cover result-to-record mapping, save/update behavior, deduplication, access counts, favorites, list loading, and detail lookup.
- Database tests will verify compatibility with existing rows and serialized record fields. No test will depend on a developer or user database file.
- Frontend store tests will reuse the existing mocked Tauri invoke pattern. They will verify simplified command payloads, reactive state updates, history merging, favorite changes, and error presentation.
- Popup tests will reuse existing runtime and component tests. They will verify staged translation events, theme changes, cleanup, Escape, drag, close, and the absence of duplicate user-visible handling.
- Existing pure tests for word detection, popup positioning, settings normalization, and history merging remain valid and should be retained.
- The project build and Rust compiler will be used to verify the new acyclic module direction. Source-text assertions about imports or module names will not be added as tests.
- Native, lite, and full OCR build configurations require smoke verification after the refactor. At minimum, each profile must resolve its intended engine resources and start without an architecture-induced initialization failure.
- A final manual smoke scenario will exercise manual translation, selected-text translation, screenshot OCR translation, popup enrichment, history persistence, favorite toggling, settings restart persistence, and rapid consecutive shortcuts.

## Out of Scope

- New user-facing translation, OCR, history, or settings features.
- UI redesign, visual restyling, or navigation changes.
- Changing the supported Youdao, Microsoft Translator, Free Dictionary, PaddleOCR, RapidOCR, or native ONNX product behavior.
- Removing documented OCR compatibility build profiles.
- Rewriting the Windows screenshot selection implementation.
- Changing the SQLite schema, database location, or user-data migration format.
- Moving settings from the existing settings file into SQLite.
- Cloud sync, accounts, multi-device behavior, telemetry, or remote persistence.
- Credential encryption or operating-system keychain integration.
- Introducing a general dependency-injection framework, plugin framework, event bus, or code-generation system.
- Performance optimization unrelated to removing duplicated workflow work.
- Broad frontend component decomposition beyond popup event ownership.
- Refactoring logging, tray behavior, autostart, routing, or theme behavior except where required to preserve the unified workflow contract.

## Further Notes

- This specification is based on the Architecture Audit baseline with a Health Score of 54/100.
- Recommended implementation order: separate translation domain and persistence records; establish neutral OCR contracts and remove cycles; introduce the unified workflow and its tests; migrate all entry points and runtime settings access; simplify popup event ownership; run all smoke scenarios; rerun the architecture audit.
- The codebase is owned by a solo developer, so no cross-team Conway's Law migration is required. The target is clearer module ownership and lower change coordination inside the repository.
- The existing application setup is allowed to retain high fan-out because it is the composition root. Business policy must not move into that setup layer.
- Complex platform implementation may remain hidden inside deep modules when their public interfaces stay small. File size alone is not a reason to split the Windows screenshot subsystem.
- Completion requires a clean dependency graph, one translation policy, one backend settings authority, backward-compatible persisted data, and verified behavior across all current entry points.
