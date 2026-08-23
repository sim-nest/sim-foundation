# Bounded capsule request

Construct a `Policy`, bind `CapsuleConnector` to the capsule's DNS and socket
ports, then call `Client::execute`. Use `execute_stream` for backpressured body
delivery. Cancellation is cooperative and dropping the response owns no live
reader because the connection is consumed and released at the boundary.
# Bounded GET

Construct a validated `Url`, choose an explicit `Policy`, and create a `Client`
with the capsule connector. The client rejects ambient redirects, proxies,
cookies, credentials, and userinfo by default. Mark authorization and cookie
headers with `Header::sensitive`; their debug representation is always redacted.

Use `execute_stream` when the consumer can process response chunks under
backpressure. Cancelling the request or returning an error from the chunk
callback immediately drops the connection.
