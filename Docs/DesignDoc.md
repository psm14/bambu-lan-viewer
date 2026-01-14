# technical design document (tdd): ios bambu x1c lan controller (phase 1)

## 0. goal

build an ios app (swiftui) that connects to a bambu x1-series printer by ip over lan/vpn, shows live rtsps camera video, displays key status (job state, temperatures, progress), and provides minimum controls: pause, resume, stop, light toggle.

phase 1 explicitly does **not** start jobs or upload gcode; jobs are initiated via desktop/sd card.

---

## 1. scope

### in scope (phase 1)

* manual add printer by ip
* store credentials (lan access code) securely
* mqtt over tls (self-signed) for status + controls
* trust-on-first-use (tofu) certificate pinning for mqtt
* rtsps live video in-app
* basic status ui:

  * connection state
  * job state: idle / printing / paused / finished / error
  * progress percent (if available)
  * nozzle/bed/chamber temps (if available)
  * light state (if available)
* controls:

  * pause
  * resume
  * stop
  * light on/off
* graceful error handling + reconnect

### out of scope (phase 1)

* job browsing/start/queue
* file upload
* printer movement/jogging
* setting target temps, fans, calibration
* discovery (bonjour), multi-printer list (optional later)

---

## 2. assumptions & constraints

* printer reachable by ip (same vlan or vpn)
* lan access code available (user enters once)
* printer uses self-signed tls cert; we will:

  * **mqtt**: tofu + pin spki hash
  * **video**: accept self-signed in the RTSP client, optionally gate video behind successful mqtt pin
* avplayer does not support rtsp/rtsps; use a native rtsp-over-tcp (interleaved) pipeline with videotoolbox decode

---

## 3. user experience

### screens

1. **printer setup**

* fields: name (optional), ip address, lan access code
* connect button
* if first connect succeeds: silently trust (tofu) and save pin
* if cert mismatch later: show “printer identity changed” alert with options:

  * cancel
  * re-trust (overwrites pin; requires explicit confirmation)

2. **printer dashboard**

* video panel (top)
* status panel (chips / rows)
* controls row (pause/resume/stop)
* light toggle
* connection indicator + last update time
* error banners (non-blocking)

### command UX

* buttons are optimistic but always reconciled by mqtt status:

  * on tap: disable button + show spinner for that command
  * success: state update arrives (paused/printing/etc.), spinner clears
  * timeout: show error, re-enable, keep listening

---

## 4. architecture

### layering

* **transport layer**: raw mqtt + video player, no swiftui, no app state
* **domain layer**: printer store, models, command semantics, trust policy
* **ui layer**: swiftui views binding to store

### concurrency model

* mqtt status updates arrive on background threads; domain layer normalizes to main actor for ui.
* domain layer uses an `actor` (or isolated class) to serialize state mutations and command sends.
* ui uses `ObservableObject` / `@Published` and `Task {}`.

---

## 5. dependencies

### mqtt

* preferred: CocoaMQTT via SwiftPM (verify current repo supports spm in package manifest)
* alternatives: mqtt-nio / custom NWConnection implementation (not needed for phase 1)

### video

* Network.framework (RTSP over TCP interleaved)
* VideoToolbox + AVSampleBufferDisplayLayer for decode/render
* swiftui integration via `UIViewRepresentable`

### storage

* Keychain (native) for lan access code and pinned cert hash
* UserDefaults for non-sensitive config (printer name, ip)

---

## 6. data model

### printer configuration

```swift
struct PrinterConfig: Codable, Equatable, Identifiable {
  var id: UUID
  var name: String
  var ip: String
  var mqttPort: Int = 8883
  var rtspsPath: String = "/streaming/live/1"
  var username: String = "bblp"
  // password stored in keychain
}
```

### state

```swift
enum ConnectionState: Equatable {
  case disconnected
  case connecting
  case connected
  case failed(message: String)
  case certMismatch
}

enum JobState: Equatable {
  case idle
  case printing
  case paused
  case finished
  case error(message: String)
}

struct PrinterState: Equatable {
  var connection: ConnectionState
  var job: JobState
  var progress01: Double?
  var nozzleC: Double?
  var bedC: Double?
  var chamberC: Double?
  var lightOn: Bool?
  var lastUpdate: Date?
}
```

### mqtt message decoding (domain-friendly)

* create `PrinterReport` DTO matching the observed mqtt payload(s)
* map `PrinterReport -> PrinterStatePatch`
* merge patch into `PrinterState`

---

## 7. mqtt topics & commands (phase 1)

* implement topic constants in one place:

  * `reportTopic(config)` (subscribe)
  * `commandTopic(config)` (publish)
* payloads for pause/resume/stop/light depend on the integration’s known structure; represent them as typed command builders:

```swift
enum PrinterCommand {
  case pause
  case resume
  case stop
  case setLight(Bool)
}
```

`MqttTransport.publish(command:payload:)` should accept a `Data` payload to keep transport generic.

---

## 8. tls trust policy (mqtt pinning)

### objective

tofu pinning to prevent silent mitm on subsequent connects while keeping first-time setup frictionless.

### policy

1. first successful tls handshake:

* accept self-signed
* extract leaf certificate’s SPKI (subject public key info)
* compute `sha256(spki)` (base64)
* store in keychain keyed by `PrinterConfig.id`

2. subsequent connects:

* on handshake, compute `sha256(spki)` again
* if matches stored: allow
* if mismatch: fail connection with `certMismatch`, require explicit “re-trust” action

### api

```swift
protocol TrustStore {
  func pinnedSPKIHash(for printerID: UUID) async -> String?
  func setPinnedSPKIHash(_ hash: String, for printerID: UUID) async
  func clearPinnedSPKIHash(for printerID: UUID) async
}
```

### mqtt transport hook

* implement a trust evaluator callback that:

  * receives server trust / cert chain (platform-specific)
  * calls domain trust policy to decide allow/deny
* if library doesn’t expose trust callback cleanly, fallback:

  * allow always at tls layer
  * detect cert changes by separately probing the cert via a lightweight tls connection before mqtt connect
  * (plan b only; prefer direct trust hook)

---

## 9. video (rtsps)

* build URL: `rtsps://{ip}{path}`
* set credentials user `bblp` pass (lan code)
* use native RTSP client:

  * interleaved RTP over TCP
  * accept self-signed tls at the Network.framework layer
  * keepalive via OPTIONS/GET_PARAMETER
* implementation details are tracked in `Docs/NativeDecoding.md`
* expose a minimal protocol:

```swift
protocol VideoTransport {
  func attach(to view: UIView)
  func play(url: URL, username: String, password: String)
  func stop()
  var onStateChanged: ((VideoState) -> Void)? { get set }
}
```

### gating

* optionally require mqtt `connected` before starting video to reduce confusion and ensure the user has entered correct lan code.

---

## 10. error handling & resilience

* exponential backoff reconnect for mqtt while app is foregrounded
* if backgrounded:

  * stop video
  * optionally pause mqtt or keep short-lived; simplest: disconnect on background, reconnect on foreground
* timeouts:

  * commands: 3–5s waiting for confirming state update
  * mqtt connect: 5–10s

---

## 11. security

* store lan access code in keychain only
* store pin hash in keychain
* never log secrets
* if vpn is used, treat similarly to lan; pinning still useful

---

## 12. testing strategy

### unit tests (domain)

* state patch merging
* command timeout behavior
* trust policy:

  * first connect stores pin
  * subsequent connect matches pin -> allow
  * mismatch -> certMismatch + requires explicit re-trust
* command mapping:

  * given a `PrinterCommand`, verify correct topic + payload bytes (golden tests)

### unit tests (transport adapters via fakes)

* `FakeMqttTransport`:

  * simulates connect/disconnect
  * emits report messages
  * records published commands
* `FakeTrustStore`:

  * in-memory
* `FakeVideoTransport`:

  * tracks play/stop calls

### ui tests (optional phase 1)

* dashboard renders offline/connecting/connected/error states
* buttons enabled/disabled based on state

### manual test plan (required)

* connect to real printer by ip on same lan
* verify pin is created on first connect
* verify pause/resume/stop work
* toggle light
* video plays and recovers after app foreground/background
* simulate cert mismatch (factory reset or clear pin + re-trust flow)

---

codex notes / guardrails

* keep all network/protocol code out of swiftui views
* keep video pipeline isolated behind VideoTransport so it can evolve independently
* treat mqtt status as source of truth; commands never directly mutate state except via “pending command” UI flags
* implement pinning first; it prevents confusing “works on my vlan” bugs from becoming silent insecurity later
