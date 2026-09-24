# 0011. World vocabulary: sparks, wishes, miracles

- **Status:** Accepted; the English terms are pending review by native speakers
- **Date:** 2026-09-24
- **Specification:** §2

## Context

The word "mining" hurts the project in three ways: it triggers antivirus products, it conflicts with app store rules, and it evokes cryptocurrency for exactly the audience that was promised "no token". The v0.1 terms "ticket" and "win" also suggested gambling. Beyond that, the player needs a fantasy: a naturalist with a small budget of miracles, not a trader at a terminal.

## Decision

The interface speaks the language of the world:

| Technical term | English | Russian |
|---|---|---|
| PoW computation | kindle sparks | разжигать искры |
| Share (`spark`) | spark | искра |
| Proposal (`proposal`) | wish | замысел |
| Executed intervention (`miracle`) | miracle | чудо |
| Price (`price`) | price of a miracle | цена чуда |
| Key holder | naturalist | натуралист |

Code and protocol fields keep their technical names. The words "mining", "miner", "token", "coin", "reward", "earn" and "win" are not used in the interface, in program names or in app store listings. The fallback for "wish" is "intent".

## Consequences

- A consistent tone across the interface, the documentation and the community; the [glossary](../glossary.md) is the reference.
- Internal names such as `miner_pubkey` remain in the protocol, where users never see them.
- Native speakers review the English terms before stage B.

## Alternatives considered

- **Technical terms in the interface** ("mining", "shares") — cold, and it carries all three risks above.
- **Religious terms** ("offerings", "prayers") — too loaded for a naturalist's world.
