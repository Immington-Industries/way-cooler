# Command Channel Packets
These are the packets used in the socket `/var/run/user/<user-id>/way-cooler/<unique-id>/command`.
Each is a JSON table, with a `type` field specifying what kind of request it is.

## Errors
In the event of an error - a malformed packet or an invalid user action - an error response will be sent.
This has the key `type` set to `error` and a short description in `reason`. All errors will have reasons.

```json
SEND { "type": "some-invalid-type" }
RECV { "type": "error", "reason": "invalid message type" }
SEND { "type": "get" }
RECV { "type": "error", "reason": "missing message field", "missing": "key", "expected": "String" }
```

# Communication
The client starts communication by sending a packet, and the server will send responses.
Replies either have `"type": "success"` or `"type": "error"`.

## get
Gets data from way-cooler. See the registry docs for a list of keys.
```json
{ "type": "get", "key": "views.current" }
{ "type": "get", "key": "mouse.coords" }
```

#### Reply
The reply will either be a `value` with the requested value, or an error.
```json
{ "type": "success", "value": 12 }
```

#### Errors
Errors will be returned if the key does not exist or is a command ("key not found"),
or the key cannot be accessed (if it is write-only; "cannot 'get' that key").

```json
SEND { "type": "get", "key": "some-invalid-key-heeeeerrrreeee" }
RECV { "type": "error", "reason": "key not found" }

SEND { "type": "get", "key": "workspace-right" }
RECV { "type": "error", "reason": "key not found" }

SEND { "type": "get", "key": "notify" }
RECV { "type": "error", "reason": "cannot 'get' that key"}
```

## set
Sets data in way-cooler. See <the registry docs> for a list of keys.
Note that at the moment new keys cannot be created, in the future an 'insert' command may be added.
```json
{ "type": "set", "key": "mouse.coords", "value": { "x": 12, "y": 22 } }
```

#### Reply
The reply will either be an empty `success` or an access error.
```json
SEND { "type": "set", "key": "mouse.coords", "value": { "x": 12, "y": 22 } }
RECV { "type": "success" }
```

#### Errors
Errors will be returned if the key does not exist or is a command ("key not found"),
or the key cannot be accessed (if it is read-only; "cannot 'set' that key").
```json
SEND { "type": "set", "key": "immutable_key", "value": "newvalue" }
RECV { "type": "error", "reason": "cannot 'set' that key" }
SEND { "type": "set", "key": "workspace-right", "value": "workspace-left" }
RECV { "type": "error", "reason": "key not found" }
SEND { "type": "set", "key": "totally a real key", "value": "totally a legit value" }
RECV { "type": "error", "reason": "key not found" }
```

## exists
Checks if a key with that name exists, and gets some metadata about it.
```json
{ "type": "exists", "key": "some_key" }
```

#### Reply
The reply will always be `success` if the `key` field was specified.
The reply will either have `"exists": true` or `"exists": false`.

If the key exists, the field `key_type` will be one of the following:
- `Object`: The value at that key is a plain old JSON.
- `Property`: The value at that key is a property, getting and setting it may involve code being executed (i.e. `mouse.coords` is a property with get/set).
- `Command`: The value at that key is a command, which can be run using `run`.

If the key is an `Object` or `Property`, it will also have a `flags` array, which may contain the following:
- `"read"`: `get` will work on the key.
- `"write"`: `set` will work on the key.

There are currently no special flags for commands.
```json
SEND { "type": "exists", "key": "some_key" }
RECV { "type": "success", "exists": true, "key_type": "Object", "flags": [ "read", "write" ] }

SEND { "type": "exists", "key": "quit" }
RECV { "type": "success", "exists": true, "key_type": "Command" }

SEND { "type": "exists", "key": "pointer.coords" }
RECV { "type": "success", "exists": true, "key_type": "Property", "flags": [ "read", "write" ] }

SEND { "type": "exists", "key": "foobar" }
RECV { "type": "success", "exists": false }

SEND { "type": "exists", "key": "screens.length" }
RECV { "type": "success", "exists": true, "key_type": "Property", "flags": [ "read" ] }
```
#### Errors
This command will only return an error if the `key` field is not specified.
```
SEND { "type": "exists" }
RECV { "type": "error", "reason": "missing message field", "missing": "key", "expected": "String" }
```

## run
Run a command. Command names are used for keybindings in the init file.
```json
{ "type": "run", "key": "workspace_move_left" }
```

#### Reply
The reply will either be `success` or an error.
```json
SEND { "type": "run", "key": "workspace_send_right" }
RECV { "type": "success" }
```

#### Errors
An error will be returned if the key does not exists or is not a command ("key not found").
```json
SEND { "type": "run", "key": "bogus_command" }
RECV { "type": "error", "reason": "key not found" }
SEND { "type": "run", "key": "mouse.coords" }
RECV { "type": "error", "reason": "key not found" }
```

## version
Gets the version of the API. Currently version 0, will hit version 1 on release,
and any changes in the API result in a version increment. Consult these docs to see
what changes between versions.

```json
SEND { "type": "version" }
RECV { "type": "success", "value": 1 }
```

## ping
Replies with a `success` packet.
```json
SEND { "type": "ping" }
RECV { "type": "success" }
```
