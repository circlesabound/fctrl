# API design

## REST

### `GET /api/server/status`

Gets the status of the Factorio server.

Response body sample:

```json
{
    "running": false
}
```

### `POST /api/server/start`

Sends a request to start the Factorio server.

Request body sample:

```json
{
    "save": "my save"
}
```

### `POST /api/server/stop`

Sends a request to stop the Factorio server.

### `GET /api/server/savefile`

Gets the savefiles present on the Factorio server.

Response body sample:

```json
[
    {
        "name": "my save",
        "date_modified": "2021-04-17T10:02:23.311Z"
    },
    {
        "name": "my save older",
        "date_modified": "2021-04-11T11:23:01.049Z"
    }
]
```

### `POST /api/server/savefile/create`

Create a new savefile with the given name.

Request body sample:

```json
{
    "name": "my new save"
}
```

### `POST /api/server/install`

Sends a request to upgrade the Factorio server to the specified version, or install if no version previously installed.

The `force_install` field sould be set to `true` if reinstalling the current version is the intent, otherwise specifying the currenly installed version is a no-op.

Request body sample:

```json
{
    "version": "1.1.32",
    "force_install": false
}
```

### `GET /api/server/config/adminlist`

Gets the adminlist the Factorio server is configured to use.

Response body sample:

```json
[
    "admin1",
    "admin2",
    "admin3"
]
```

### `PUT /api/server/config/adminlist`

Pushes an adminlist to the Factorio server for use.

Request body sample:

```json
[
    "admin1",
    "admin2"
]
```

### `GET /api/server/config/rcon`

Gets the RCON configuration used by the Factorio server.

Response body sample:

```json
{
    "port": 27015,
    "password": "rconpassword123"
}
```

### `PUT /api/server/config/rcon`

Pushes an RCON configuration to the Factorio server for use. NOTE: port cannot be modified.

Request body sample:

```json
{
    "password": "newrconpassword"
}
```

### `GET /api/server/config/server-settings`

Gets the server-settings file used by the Factorio server.

Response body sample:

```json
{
    "name": "Name of the game as it will appear in the game listing",
    "description": "Description of the game that will appear in the listing"
}
```

### `PUT /api/server/config/server-settings`

Pushes a server-settings file to the Factorio server for use.

Request body sample:

```json
{
    "name": "My awesome server",
    "description": "Let's have some fun!"
}
```

### `GET /api/server/mods`

Gets a list of mods installed on the Factorio server.

Response body sample:

```json
[
    {
        "name": "rso-mod",
        "version": "6.2.4",
    },
    {
        "name": "stdlib",
        "version": "1.4.6"
    }
]
```

## Websockets

### `/ws/logs`

Request a websocket connection to access logs sent from the agent.
