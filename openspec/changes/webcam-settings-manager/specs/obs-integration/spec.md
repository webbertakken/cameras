## ADDED Requirements

### Requirement: OBS websocket connection

The system SHALL connect to a running OBS Studio instance via the obs-websocket v5 protocol, allowing bidirectional communication.

#### Scenario: OBS is running with websocket enabled

- **WHEN** the user enables OBS integration and provides the websocket address and password
- **THEN** the system connects to OBS and reports a successful connection status

#### Scenario: OBS is not reachable

- **WHEN** the user enables OBS integration but OBS is not running or the address is wrong
- **THEN** the system shows a connection error with a clear message
- **AND** the system retries periodically in the background

### Requirement: Camera source coordination

The system SHALL be able to read and set properties on OBS video capture sources, coordinating camera settings between the app and OBS.

#### Scenario: User views OBS camera sources

- **WHEN** connected to OBS
- **THEN** the system can list OBS scenes and their video capture sources

#### Scenario: User switches camera in OBS from the app

- **WHEN** the user selects a different camera in the webcam settings manager
- **THEN** the corresponding OBS video capture source can be updated to match (with user confirmation)

### Requirement: OBS connection persistence

The system SHALL remember the OBS websocket connection details and auto-connect on launch if OBS integration was previously enabled.

#### Scenario: App launches with OBS integration enabled

- **WHEN** the app starts and OBS integration was previously configured
- **THEN** the system attempts to connect to OBS automatically using saved credentials
