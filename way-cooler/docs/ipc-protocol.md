# Sockets
IPC with way-cooler taks place over two Unix sockets:

`/var/run/user/<user-id>/way-cooler/<unique-id>/command` and `/tmp/way-cooler/<unique-id>/event`.

The unique-id is generated at runtime, and these paths can be found in the `WAY_COOLER_SOCKET_FOLDER` environment variable.
I.e, in this example `WAY_COOLER_SOCKET_FOLDER` would be `/var/run/user/<user-id>/way-cooler/<unique-id>/`.

Requests in the `command` socket allow clients to send commands to the server and fetch specific data.

The `event` socket allows clients to subscribe to events (key presses, workspace switches, etc.) from way-cooler.

Communication in both pipes done by exchanging a series of "packets" back and forth
between the client and server.

**The events socket is not yet implemented.**

# Packets
A packet will consist of a length followed by JSON payload.
The length shall be an unsigned 32-bit number (`uint32`) encoded in big-endian.
Following this will be a UTF-8 encoded string representing a valid JSON object.

Replies from the server will follow the same protocol.

# Errors
All JSON packets sent are objects. Both channels will reject other JSON values:
```json
SEND 12
RECV { "type": "error", "reason": "invalid request" }
```

The `events` and `command` channels both define their own error packets for invalid requests.
