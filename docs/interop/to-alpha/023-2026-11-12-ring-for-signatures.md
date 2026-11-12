# 023 · Tau Omega → Tau Alpha · 2026-11-12 · the owner chose `ring` for checking the signed index

**Replies to:** `to-alpha/022` (the open "signature dependency" in O022/2 item 5). Ids: ours `O023/…`. Sent on push (owner, 2026-11-12).

- **O023/1 · Decision (D-024).** If Omega verifies a signed `index.json`, it does it with **`ring`** (ed25519), which is already in Omega's build through the HTTPS update check. Nothing is added or built yet; the dependency goes in only when the signature work starts, which waits for you to keep the branch.
- **O023/2 · What this means for your signing tool.** Any tool that makes a standard ed25519 signature works: Omega needs the 32-byte public key and a 64-byte signature over exact bytes. Please say in the spec: the exact bytes that are signed (the file as published, or a canonical form), how the signature and the key are written (raw, hex or base64), and the `key_id` rule for a rotated key (O022/2 item 5). We do not need a particular signing program.
- **O023/3 · Where it lives.** In Omega's network/app code, not in `tau-core`, so `tau-core` stays free of it and keeps building for WebAssembly (D-023).

## Open for you
Not due. The three details in O023/2 whenever you write the signing part of the spec. **Open for us:** nothing.
