# implementation checklist

## a) project setup

* [x] create new ios app (swiftui) with minimum ios target required by your device fleet
* [x] add `NSLocalNetworkUsageDescription` to info.plist
* [x] add swiftpm dependency for mqtt (CocoaMQTT preferred)
* [x] add native rtsp/video pipeline scaffolding:

  * [x] verify rtsps playback in a tiny spike view controller
* [x] create `AppEnvironment` container for dependency injection (simple struct or singleton)

## b) core types

* [x] implement `PrinterConfig`, `PrinterState`, enums
* [x] implement `PrinterCommand`
* [x] define protocols:

  * [x] `MqttTransport`
  * [x] `VideoTransport`
  * [x] `TrustStore`
  * [x] `SecretStore` (keychain wrapper for lan code)

## c) storage

* [x] implement `KeychainSecretStore`:

  * [x] save lan access code per printer id
  * [x] read lan access code
* [x] implement `KeychainTrustStore`:

  * [x] get/set/clear pinned spki hash
* [x] implement `PrinterConfigStore` (UserDefaults or small JSON file):

  * [x] save/load last-used printer config

## d) mqtt transport adapter (real)

* [ ] implement `CocoaMqttTransport`:

  * [x] connect(ip, port, username, password, tls)
  * [x] subscribe(reportTopic)
  * [x] publish(commandTopic, payload)
  * [x] expose async stream / callback for incoming report messages
  * [ ] implement reconnect hooks
  * [x] connection state callbacks
* [x] implement tls trust hook:

  * [x] on handshake: extract spki hash
  * [x] consult trust store:

    * [x] no pin -> store pin and allow
    * [x] pin matches -> allow
    * [x] mismatch -> deny and surface certMismatch

## e) report decoding + state mapping

* [x] create `PrinterReport` DTO matching observed mqtt JSON
* [x] implement `PrinterStatePatch`
* [x] implement `apply(patch:)` merge into `PrinterState`
* [ ] write unit tests for:

  * [ ] merge rules (e.g., preserve old values when patch is nil)
  * [ ] job state transitions

## f) domain service + store

* [ ] implement `actor PrinterService`:

  * [x] holds config + transports + stores
  * [x] `connect()`, `disconnect()`
  * [x] command methods: `pause()`, `resume()`, `stop()`, `setLight(_:)`
  * [x] subscribes to mqtt reports and updates internal state
  * [ ] command timeout logic (await state confirmation or timeout)
* [ ] implement `@MainActor PrinterStore: ObservableObject`:

  * [x] `@Published state`
  * [x] forwards methods to service via Tasks
  * [ ] handles app lifecycle notifications:

    * [x] pause/resume video on scene phase changes
    * [ ] connect/disconnect mqtt on foreground/background (optional)

## g) video adapter (real)

* [x] implement native RTSP pipeline:

  * [x] RTSP client (TLS, auth, keepalive)
  * [x] interleaved RTP demux + H264 depacketizer
  * [x] VideoToolbox decode + AVSampleBufferDisplayLayer rendering
  * [x] state callbacks for buffering/playing/stopped/error
* [x] implement swiftui wrapper:

  * [x] `RTSPPlayerView: UIViewRepresentable` that creates hosting UIView and attaches player
  * [x] exposes `url`, `username`, `password`, `isActive`

## h) ui

* [ ] `AddPrinterView`:

  * [x] name, ip, lan code inputs
  * [x] connect button
  * [x] on success: navigate to dashboard
  * [ ] on certMismatch: show re-trust prompt
* [ ] `DashboardView`:

  * [x] video panel (with overlay for connection state)
  * [x] status panel (job state, progress, temps)
  * [ ] controls row:

    * [x] pause enabled only when printing
    * [x] resume enabled only when paused
    * [x] stop enabled when printing or paused
  * [x] light toggle (if light state known; otherwise show toggle as “send command”)
  * [ ] error banner + retry connect
* [ ] add simple printer selector later (optional)

## i) tests

* [ ] create fakes:

  * [ ] `FakeMqttTransport` (scriptable inbound messages + records publishes)
  * [ ] `FakeVideoTransport`
  * [x] `InMemoryTrustStore`, `InMemorySecretStore`
* [ ] unit tests:

  * [ ] tofu pinning behavior
  * [ ] command publishing + timeout
  * [ ] state mapping from sample report json fixtures
* [ ] integration smoke:

  * [ ] connect to real printer
  * [ ] verify pause/resume/stop
  * [ ] verify video plays
  * [ ] verify background/foreground handling

## j) polish (phase 1)

* [x] add “last update” timestamp
* [ ] show stale data warning after N seconds
* [ ] add retry/backoff strategy for mqtt connect
* [ ] add log levels with redaction (no secrets)
* [ ] add settings: “auto-connect on launch”, “auto-start video”, “clear trust (re-trust)”
