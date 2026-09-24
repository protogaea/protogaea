# Privacy

> **Draft.** These are design principles, not a legal privacy policy. A formal policy will be written and reviewed before any public service launches.

## Principles

- Watching requires no account and no personal data.
- Keys are pseudonymous. Nothing links a key to your identity unless you do so yourself.
- The world's history is public and permanent by design: that is what makes it verifiable.

## Public forever

These records are part of the verifiable log and cannot be deleted later:

- **Wishes:** the author's public key, the action and its parameters, the attached hypothesis and name indexes.
- **Sparks:** the public key of the naturalist who produced each spark and the wish it supports, together with the spark log's signed tree heads.
- **Standalone predictions:** signed with your key; their hashes are anchored in time.
- **Everything in the simulation:** organisms, clades, events, miracles.

A published rule can hide a record from display, but it stays in the log ([spec §25](spec/spec-v0.2.md#25-operations-load-abuse)). If you want your activity to be unlinkable, use a separate key.

## Stored privately, and for how long

| Data | Why | How long |
|---|---|---|
| IP addresses | Rate limiting and blocking abuse | At most 7 days |
| Notification endpoints (web push) or Telegram chat IDs | Delivering the notifications you asked for | While you are subscribed; deleted when you unsubscribe |
| Rings and subscriptions | Your digest and notifications | Until you remove them |
| Server logs | Operations and debugging | Short retention (TBD) |

## Never

- No advertising and no third-party trackers.
- We never sell or share personal data.
- No email address or phone number is required.

## On your device

- The "While you were away" digest for anonymous viewers relies on the time of your previous visit, stored in your browser.
- Your key is stored on your device: in the operating system's keychain in the desktop app, or as a non-extractable key in the browser where supported. We cannot recover a lost key. Keep the encrypted backup.

## Open questions

- **Product metrics.** Stages B′ and D measure return rates, preferably with aggregated, cookieless analytics. The exact method is TBD and will be published before the test.
- **Server log retention:** TBD.
- **Personal-data deletion requests:** how they apply to a permanent public log, TBD with legal review.
