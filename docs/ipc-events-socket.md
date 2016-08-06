# Events
By connecting to an event channel, clients can recieve envents from way-cooler.

## Requesting
The first packet set across the event pipe should be a list of events requested.
```json
{ "type": "request", "events": [  { "type": "key.pressed", "keys": [ "ctrl", "alt", "delete" ] }, "workspace.switched" ] }
```
If the request packet is formatted properly, the server wil send a `{ "type": "success" }` packet.
So for example, the above request will notify the client whenever the user hits ctrl+alt+delete, or when they switch workspaces

### Errors
The reply will be an error if the JSON was invalid or an event was requested which does not exist.
The client may re-attempt to register events after receiving an error.

## Event loop
After the server sends the success packet, it will send events. The server will not listen for inputs and only send more packets.

### Event packets
Event packets are described in the <event docs>. The general format:
```json
{ "type": "event", "name": "event_name", "field1": "value",  }
```
