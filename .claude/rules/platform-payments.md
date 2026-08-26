---
paths:
  - "__agent_only_never_match_at_startup__/**"
---

# Platform Payment Systems

A reference for reviewing code that charges users money: Apple StoreKit, Google Play Billing, web and third-party processors, and the cross-platform entitlement architecture that ties them together. Used by the `platform-payments` subagent.

Distinct from:
- **`security`**: general threat model. We own payment-specific trust boundaries -- client-asserted entitlement, receipt forgery, webhook signature verification.
- **`api-design`**: consumer-contract design generally. We own the entitlement contract and webhook handler shape.
- **`product-leadership`**: pricing strategy, packaging, and growth. We own the mechanics and the policy gate.
- **`distsys-runtime`**: retries and idempotency at the service layer. We own webhook idempotency and reconciliation specifically.
- **`platform-release`**: store submission generally. We own the payment-related rejection surface.
- **`app-privacy-compliance`**: regulatory obligation generally. We touch consent for consumption data and PII in purchase identifiers.

The core thesis: **entitlement is server state, not client state.** Every value-granting decision must resolve on a server that has independently verified the purchase with the platform. The strongest argument is not tampering -- it is **freshness**. A cryptographically perfect, correctly signed transaction says nothing about a refund granted an hour later. That holds even for entirely honest clients on stock hardware.

The operational priority: **find the durable identifier and the entitlement source of truth.** Ask what key the subscription is stored under, and what the client does when the server is unreachable. Most serious defects in this domain are visible from those two answers.

**Currency of information matters here more than in any other lens.** Commission rates, anti-steering rules, and library deadlines changed repeatedly through 2025 and 2026 and are under active litigation. Confidence markers below: **[V]** verified against a primary source, **[V2]** corroborated secondary sources only, **[U]** unverified and flagged. Re-verify anything with a date before relying on it commercially.

---

## Universal principles

### Server-authoritative entitlement

```
Store event (ASSN V2 / RTDN / Stripe webhook)
  -> verify signature
  -> fetch authoritative state from the platform API
  -> upsert entitlement keyed by (platform, durable_id)
  -> resolve to your user via the account link
  -> clients READ entitlement from your API
```

**Durable keys**: Apple `originalTransactionId`; Google `purchaseToken` (**not `orderId`** -- promo-code purchases have no `orderId` [V]); Stripe subscription ID.

Caching for offline experience is fine with a short time-to-live. A client-side `isSubscribed` boolean as the decision point is not.

### The notification is a trigger, not a source of truth

All three platforms deliver at-least-once and unordered. Google states the re-fetch requirement outright; Stripe explicitly disclaims ordering [V].

Two safe patterns: treat every event as a **trigger to re-fetch authoritative state** (preferred), or store a monotonic version per entitlement and **drop stale writes**. Never apply an event as a blind state transition.

### Reconciliation is mandatory, not optional

Notifications get lost: endpoint outages, misconfigured subscriptions, silent 5xx responses. **Pure event-driven with no reconciliation drifts and stays drifted**, with no self-healing.

Run a periodic job over subscriptions expiring soon and re-query the platform. Apple provides **Get Notification History** specifically for backfill [V]. Poll Google's **Voided Purchases API**, since chargebacks may not otherwise reach you.

### Cancelled is not expired

Apple `DID_CHANGE_RENEWAL_STATUS` with `AUTO_RENEW_DISABLED`, and Play `SUBSCRIPTION_CANCELED`, mean auto-renew is off. **Entitlement continues to the paid-through date.** Revoking on cancellation steals paid time and generates support load.

Likewise **grace period is entitled; billing retry and account hold are not.**

### Refunds run in both directions

`REFUND_REVERSED` means a dispute overturned the refund and the customer legitimately paid. **A one-way revoke path locks out a paying customer.**

---

## Apple StoreKit

### StoreKit 1 is deprecated but not removed

`SKPaymentQueue`, `SKPaymentTransactionObserver`, and `SKProductsRequest` are deprecated as of iOS/iPadOS/tvOS 18.0, macOS 15.0, watchOS 11.0, visionOS 2.0 [V]. `SKPaymentQueue`'s summary reads "No longer supported"; the observer's reads "Use StoreKit 2 Transaction APIs."

**No removal date has been announced [V].** Review posture: new StoreKit 1 code is a finding; existing StoreKit 1 code is debt with no hard deadline.

StoreKit 2 requires iOS 15.0+, macOS 12.0+ [V], and is Swift-only.

### Two structural traps

**`RenewalState` is a struct with static properties, not an enum [V].** Members: `.subscribed`, `.expired`, `.inBillingRetryPeriod`, `.inGracePeriod`, `.revoked`. Consequences: a `switch` is **not** exhaustiveness-checked, a `default` is mandatory to compile, and **adding a new state produces no compiler error anywhere**. Every switch needs an explicit default that **denies rather than grants**. The same applies to `Transaction.OfferType`.

**Prices are integer milliunits [V].** `price` and `renewalPrice` are in thousandths. Treating them as decimal currency is a 1000x error, and it is frequent.

### Verification results carry a payload in both cases

`VerificationResult<T>` is `.verified(T)` or `.unverified(T, VerificationError)`. **The payload is available in both cases**, so `try? result.payloadValue` silently accepts forged data. Check the case, not just the presence of a value.

### Transaction.updates must be attached at launch

Ask to Buy approvals, strong-authentication deferrals, and interrupted purchases arrive asynchronously, **possibly days later and in a different app session**. Attach `Transaction.updates` at app launch outside any view lifecycle. Granting entitlement only inline after `purchase()` returns loses these purchases: the user paid and got nothing.

`AppStore.sync()` prompts for Apple ID credentials. Call it **only** when the user explicitly asks; calling it at launch reads as phishing and draws reviewer attention.

### Identifiers

- **`originalTransactionId`** -- durable across renewals, upgrades, and restores. **The correct primary key.**
- `transactionId` -- changes every renewal. Using it as the subscription key makes each renewal look like a new subscription, duplicating rows and corrupting churn metrics.
- `webOrderLineItemId` -- per billing period.
- **`appAccountToken`** (UUID) -- your user-ID linkage, set at purchase via `Product.PurchaseOption.appAccountToken`. **No longer purchase-time-only**: the Set App Account Token endpoint (v1.16, 2025-06-09) lets your server set it for out-of-app purchases such as offer-code redemptions [V]. **It is rejected for `FAMILY_SHARED` transactions [V]** -- family-shared entitlements cannot be linked this way, which is a genuine architectural constraint, not an oversight.

### Server API

**`verifyReceipt` is deprecated [V]; no sunset date has ever been published [V].** Treat any specific shutdown date as fabricated. App Store Server Notifications V1 is likewise deprecated in favor of V2.

**The base domain changed 2026-05-05 [V]**: `api.storekit.itunes.apple.com` became `api.storekit.apple.com` (sandbox correspondingly). Old domains still work; a hardcoded old domain is working-but-stale.

Key endpoints [V]: Get Transaction History `GET /inApps/v2/history/{anyTransactionId}`; Get All Subscription Statuses `GET /inApps/v1/subscriptions/{anyTransactionId}`; **Finish Transaction** `POST /inApps/v1/transactions/{transactionId}/finish` (v1.20, 2026-04-13); **Set App Account Token** `PUT /inApps/v1/transactions/{originalTransactionId}/appAccountToken` (v1.16); Send Consumption Information `PUT /inApps/v2/transactions/consumption/{transactionId}`; Get Notification History; Look Up Order ID; Extend Subscription Renewal Date (individual and bulk).

Deprecated: Get Transaction History V1, Get Refund History V1.

Authorization uses an ES256-signed JWT from an App Store Connect API key [U -- exact claim structure and rate limits not verified].

### Notification types

**All 22 `notificationType` values [V]**: `CONSUMPTION_REQUEST`, `DID_CHANGE_RENEWAL_PREF`, `DID_CHANGE_RENEWAL_STATUS`, `DID_FAIL_TO_RENEW`, `DID_RENEW`, `EXTERNAL_PURCHASE_TOKEN`, `EXPIRED`, `GRACE_PERIOD_EXPIRED`, `METADATA_UPDATE`, `MIGRATION`, `OFFER_REDEEMED`, `ONE_TIME_CHARGE`, `PRICE_CHANGE`, `PRICE_INCREASE`, `REFUND`, `REFUND_DECLINED`, `REFUND_REVERSED`, `RENEWAL_EXTENDED`, `RENEWAL_EXTENSION`, `RESCIND_CONSENT`, `REVOKE`, `SUBSCRIBED`, `TEST`.

`METADATA_UPDATE`, `MIGRATION`, `PRICE_CHANGE`, and `RESCIND_CONSENT` are Advanced Commerce API only [V].

**There is no `INITIAL_BUY` notification type.** `INITIAL_BUY` is a **subtype of `SUBSCRIBED`** [V]. Handler code comparing `notificationType == "INITIAL_BUY"` matches nothing and **silently drops every first-time subscription**. `RESUBSCRIBE` is likewise a subtype.

**Two confusable types [V]**: `RENEWAL_EXTENDED` is an individual extension with **no subtype**; `RENEWAL_EXTENSION` is the bulk operation with subtype `SUMMARY` or `FAILURE`.

Subtype-to-parent mapping [V]:

| Subtype | Parent type |
|---|---|
| `INITIAL_BUY`, `RESUBSCRIBE` | `SUBSCRIBED` |
| `UPGRADE`, `DOWNGRADE` | `DID_CHANGE_RENEWAL_PREF`, `OFFER_REDEEMED` |
| `AUTO_RENEW_ENABLED`, `AUTO_RENEW_DISABLED` | `DID_CHANGE_RENEWAL_STATUS` |
| `GRACE_PERIOD` | `DID_FAIL_TO_RENEW` |
| `BILLING_RECOVERY` | `DID_RENEW` |
| `VOLUNTARY`, `BILLING_RETRY`, `PRICE_INCREASE`, `PRODUCT_NOT_FOR_SALE` | `EXPIRED` |
| `PENDING`, `ACCEPTED` | `PRICE_INCREASE` |
| `SUMMARY`, `FAILURE` | `RENEWAL_EXTENSION` |
| `UNREPORTED`, `ACTIVE_TOKEN_REMINDER` | `EXTERNAL_PURCHASE_TOKEN` |

`notificationUUID` is the idempotency key. Nested JWS payloads (`signedTransactionInfo`, `signedRenewalInfo`) must be verified **independently against Apple's certificate chain via the x5c header, including revocation and chain-to-root** -- not merely base64-decoded.

### Proration

From Apple's own event tables [V]:

| Change | Timing | Notification |
|---|---|---|
| Upgrade | **Immediate**, new period, **prorated refund** | `DID_CHANGE_RENEWAL_PREF` + `UPGRADE` |
| Downgrade | **At next renewal** | `DID_CHANGE_RENEWAL_PREF` + `DOWNGRADE` |
| Crossgrade, same duration | Immediate | subtype `UPGRADE` |
| Crossgrade, different duration | At renewal | subtype `DOWNGRADE` |
| Reverting a pending downgrade | Cancels the change | `DID_CHANGE_RENEWAL_PREF`, **empty subtype** |

The empty-subtype case means "the customer cancelled their downgrade" and is easy to miss.

### Consumption requests

Fired for **all product types**, not just consumables [V]. **12-hour response window [V].** **If the customer did not consent, do not call the endpoint at all [V].**

**V1 and V2 field shapes differ [V]** and this is a real migration trap. V2 `ConsumptionRequest` has roughly five fields (`customerConsented`, `deliveryStatus`, `refundPreference`, `sampleContentProvided`, and a consumption measure). The familiar large form -- `accountTenure`, `lifetimeDollarsPurchased`, `playTime`, `userStatus` -- belongs to the **deprecated V1 endpoint**. Code written against V1 names posted to V2 is malformed, and missing the window forfeits your input to Apple's refund decision.

### Sandbox

**Renewal rate is configurable, not a fixed table [V].** At the 5-minute default, a monthly subscription renews every 5 minutes and an annual every hour. **Sandbox subscriptions renew up to 12 times, then auto-renew turns off on the 13th attempt [V]** (not 6).

**Xcode StoreKit Testing** with local `.storekit` files and `SKTestSession` runs entirely on-device and **emits no App Store Server Notifications**, so it cannot exercise your webhook path. That requires sandbox, which posts to a **separately configured sandbox URL**.

TestFlight uses the sandbox environment: purchases are free and durations accelerated.

### Commission

**Apple publishes no consolidated rate card**, so these are **[V2]**: 30% standard; **15% Small Business Program** [V] for developers under $1M USD proceeds in the prior calendar year across all associated accounts, with crossing the threshold mid-year applying prospectively rather than retroactively [V2]; 15% on auto-renewable subscriptions after 12 months of continuous paid service by the same subscriber [V2]. Exact year-2 conditions around lapses and resubscription are **[U]**.

### The 2025-2026 regulatory situation

**This is the fastest-moving area in the lens. Every claim is dated, and re-verification is required before commercial reliance.**

**United States, Epic v. Apple:**
- 2025-04-30: the district court found Apple in civil contempt and barred any commission on external web purchases [V2].
- 2025-05-02: Apple updated the guidelines; US-storefront apps can link out without an entitlement [V].
- The Ninth Circuit upheld the contempt finding but **reversed the outright commission ban**, remanding for a rate limited to costs genuinely necessary for coordinating external links [V2].
- **2026-06-30: the Supreme Court granted certiorari** (No. 25-1311), limited to whether civil contempt may rest on violating an injunction's "spirit" where it is silent on the conduct [V]. Apple's brief was due 2026-09-14 [V].
- **2026-08-13/14: Apple proposed US link-out commissions of 15% standard, 10% for partner programs and subscription renewals, 5% for Small Business [V2]. These are proposed and pending court approval; Apple currently collects nothing on US link-outs [V2].**

**Practical guidance: US external-link commission is 0% today but under active litigation with a rate proposal pending. Any architecture assuming permanent 0% is speculative.**

**European Union, DMA -- restructured, announced 2026-08-18, effective 2026-10-01 [V]:**

The **Core Technology Fee, Initial Acquisition Fee, and Store Services Fee are all eliminated**, replaced by a **5% Core Technology Commission** on digital transactions for alternative marketplaces and Web Distribution [V].

| Payment path | Standard | Reduced |
|---|---|---|
| Apple in-app purchase | 26% | 15% |
| Alternative in-app processing | 20% | 10% |
| Link-out to web | 15% | 10% |

Link-out commission applies only to sales within **7 days of the link tap** [V]. **Anyone citing the Core Technology Fee as current is now wrong.**

Japan's MSCA, South Korea, India, Brazil, and the UK DMCCA are **[U]** -- do not assert their current state.

---

## Google Play Billing

### Version cadence forces upgrades

**Current: Play Billing Library 9.1.0 (2026-06-18) [V].** A **two-year deprecation cadence** [V] blocks publishing new apps and updates on old versions, though already-published binaries keep transacting:

| Version | Deadline for new apps and updates | Extension |
|---|---|---|
| 7 | **2026-08-31** | 2026-11-01 |
| 8 | 2027-08-31 | 2027-11-01 |
| 9 | 2028-08-31 | 2028-11-01 |

### The v8 migration footgun

**v7's no-argument `enablePendingPurchases()` implicitly enabled one-time-product pending support. v8 requires it explicitly [V]:**

```kotlin
PendingPurchasesParams.newBuilder().enableOneTimeProducts().build()
```

Migrating without adding `.enableOneTimeProducts()` **silently disables cash and pending payments** in markets like Japan and Germany. It compiles, throws nothing, and quietly loses revenue. **This is the highest-value single v8 review check.**

Also removed in v8 [V]: `queryPurchaseHistoryAsync()`, `querySkuDetailsAsync()`, the `queryPurchasesAsync(String, ...)` overload. `enableAlternativeBilling()` became `enableUserChoiceBilling()`.

`queryProductDetailsAsync`'s callback changed from a plain list to `QueryProductDetailsResult`, which **splits fetched from unfetched products with per-product status codes** [V]. Previously missing products vanished silently, and v8's multi-offer one-time products make partial failure far more likely.

`queryPurchaseHistoryAsync()` has **no client-side replacement** [V]; Google's position is that on-device history is not authoritative. Use `purchases.subscriptionsv2.get`, notification-built history, and the Voided Purchases API.

### Acknowledgment is the biggest footgun

**Acknowledge within 3 days or Google automatically refunds and revokes [V].**

**Prepaid plans are shorter [V]**: plans of a week or more get 3 days; **plans under a week get half the plan duration**, so a 3-day plan allows 1.5 days. Each top-up needs acknowledgment too.

`consumeAsync()` implicitly acknowledges, so consumables need consume, not both. Prefer server-side acknowledgment, since the client can be killed between granting and acknowledging.

**Google explicitly warns against relying on non-acknowledgment as an implicit refund mechanism [V]** -- it is ambiguous. Refund explicitly via `Orders:refund` with `revoke=true`.

### Purchase state and keys

`PurchaseState`: `PURCHASED` (1), `PENDING` (2), `UNSPECIFIED_STATE` (0).

**Granting on `PENDING` is a fraud hole [V].** Pending arises from cash payments (Japanese convenience stores, German bank instruments) and delayed authorization. **The user has not paid.** Grant only on `PURCHASED`.

**`purchaseToken` is the primary key, not `orderId` [V]** -- promo-code purchases generate no `orderId`, so keying on it breaks on every promotional redemption.

`obfuscatedAccountId` and `obfuscatedProfileId` feed Google's fraud detection and link purchases to your users; they must not contain personal data, and they return server-side as `externalAccountIdentifiers`.

### Base plans, offers, and replacement modes

The old SKU model became **subscription -> base plans -> offers**. Base plans are auto-renewing or **prepaid** (extended by top-up, always `CHARGE_FULL_PRICE`). Offers are introductory, upgrade, or winback, carry `offerTags`, and contain **pricing phases** with `RecurrenceMode` of `INFINITE_RECURRING`, `FINITE_RECURRING`, or `NON_RECURRING`. **`offerToken` is mandatory** in `BillingFlowParams` for subscriptions.

Replacement modes (renamed from proration modes) [V]: `WITH_TIME_PRORATION` (default), `CHARGE_PRORATED_PRICE`, `CHARGE_FULL_PRICE`, `WITHOUT_PRORATION`, `DEFERRED`, and `KEEP_EXISTING` (8.1.0).

### The upgrade-chain double-grant

Upgrades issue a **new** `purchaseToken`, with the subscription carrying **`linkedPurchaseToken`** pointing at the old one [V]. **You must invalidate the entitlement attached to the linked token**, transitively, or both look active and a refund on one leaves the other granting access.

**`DEFERRED` compounds this [V]**: the new token is surfaced **immediately** and emits `SUBSCRIPTION_PURCHASED` right away, while the user keeps the old plan until renewal. Granting on that notification grants the upgrade early and free.

### Real-time developer notifications

The `DeveloperNotification` envelope carries **exactly one** of `subscriptionNotification`, `oneTimeProductNotification`, `voidedPurchaseNotification`, `pendingRefundReviewNotification`, or `testNotification` [V]. The pending-refund-review variant is easy to miss.

Subscription notification types [V]: 1 `SUBSCRIPTION_RECOVERED`, 2 `SUBSCRIPTION_RENEWED`, 3 `SUBSCRIPTION_CANCELED`, 4 `SUBSCRIPTION_PURCHASED`, 5 `SUBSCRIPTION_ON_HOLD`, 6 `SUBSCRIPTION_IN_GRACE_PERIOD`, 7 `SUBSCRIPTION_RESTARTED`, 8 `SUBSCRIPTION_PRICE_CHANGE_CONFIRMED` (deprecated), 9 `SUBSCRIPTION_DEFERRED`, 10 `SUBSCRIPTION_PAUSED`, 11 `SUBSCRIPTION_PAUSE_SCHEDULE_CHANGED`, 12 `SUBSCRIPTION_REVOKED`, 13 `SUBSCRIPTION_EXPIRED`, 17 `SUBSCRIPTION_ITEMS_CHANGED`, 18 `SUBSCRIPTION_CANCELLATION_SCHEDULED`, 19 `SUBSCRIPTION_PRICE_CHANGE_UPDATED`, 20 `SUBSCRIPTION_PENDING_PURCHASE_CANCELED`, 22 `SUBSCRIPTION_PRICE_STEP_UP_CONSENT_UPDATED`.

**Values 14, 15, 16, and 21 are absent. Do not assume contiguity.**

Delivery is at-least-once and unordered; deduplicate on the Pub/Sub `messageId`.

### Subscription state

`subscriptionState` [V]: `_PENDING`, `_ACTIVE`, `_PAUSED`, `_IN_GRACE_PERIOD`, `_ON_HOLD`, `_CANCELED`, `_EXPIRED`.

**Grant** for `_ACTIVE`, `_IN_GRACE_PERIOD`, and `_CANCELED` (until expiry). **Deny** for `_ON_HOLD`, `_PAUSED`, `_EXPIRED`, `_PENDING`.

**Grace-period and account-hold durations are configurable in Play Console [V]** -- hardcoding 30 days is wrong.

### Testing requires a published artifact

Billing resolves against the artifact published to a track with a **matching version code and signature**, and the tester must be opted in. A locally-installed debug build cannot transact. "It works in release but not on my machine" is expected here, not a bug.

### Billing Choice

**Launched 2026-06-30 for the EEA, UK, and US [V]**, splitting fees into a **service fee** that applies regardless of payment method (10% on the first $1M annual earnings and on all auto-renewing subscriptions, tiered above) and a **billing fee** of 5% that applies only when using Google Play's payment system [V].

A **separate Japan-only external payments program** exists requiring library 8.3+ and side-by-side choice screens [V]. Do not conflate the two. Epic v. Google specifics are **[U]**.

---

## Web and third-party payments

### Stripe

Subscription statuses [V]: `trialing`, `active`, `incomplete` (**23 hours** to complete first payment), `incomplete_expired`, `past_due`, `canceled`, `unpaid`, `paused`.

**`active` does not mean all invoices are paid [V]** -- outstanding invoices can remain open. **Revoke on `unpaid` or `canceled`, not `past_due`**, since Smart Retries may still recover it.

**A breaking change worth flagging [V]**: as of API version `2025-03-31.basil`, Invoice no longer exposes `subscription` directly; it moved to **`parent.subscription_details.subscription`**. Code expanding `subscription` on an Invoice **silently returns null** after a version bump, breaking refund-to-subscription attribution.

### Webhook discipline

- **Signature**: `Stripe-Signature` carries `t=` and `v1=`, an HMAC over `"{timestamp}.{raw_body}"`. **Ignore any scheme that is not `v1`** (a `v0` scheme is sent for test events). Use constant-time comparison. **The raw body is required** -- any middleware that reparses and reserializes JSON breaks verification, and the usual "fix" is disabling verification entirely, which converts the endpoint into an unauthenticated one that accepts forged `invoice.paid` events.
- **Default tolerance is 5 minutes. Never set 0 -- Stripe documents that this disables the recency check entirely [V]**, which is the opposite of the intent.
- **Retries run up to 3 days** with backoff [V].
- **No ordering guarantee [V].** `invoice.paid` can precede `customer.subscription.created`.
- **Return 2xx before doing work.** On `invoice.created` specifically, a non-2xx **delays finalization of all auto-collection invoices for up to 72 hours [V]**, halting billing entirely.
- Deduplicate on event ID; use `Idempotency-Key` on payment POSTs.

**Strong customer authentication**: authenticate once at signup with `setup_future_usage`, charge renewals `off_session: true`. When the issuer still demands authentication, the invoice emits **`invoice.payment_action_required`** and the subscription goes `past_due`. Handling this only in an inline browser flow strands renewals -- an email or deep-link recovery path is required.

### PCI scope

| Integration | Scope [V] |
|---|---|
| Checkout, Elements, Payment Element, mobile SDKs | **SAQ A** |
| Card fields in your own form | **SAQ A-EP** |
| Raw card number posted from your server | **SAQ D** |

Safe to store: brand, last four, expiry, and anything the API returns [V]. Never the full number or the security code.

### Merchant of record

The merchant of record is the legal seller, owing sales tax and VAT, appearing on the statement, and owning chargeback liability.

Paddle charges 5% + 50 cents [V]. **Lemon Squeezy was acquired by Stripe and is being folded into Stripe Managed Payments** [V], which entered public preview in February 2026 at 5% + $0.50 [V]. **RevenueCat is not a merchant of record for app-store purchases** -- Apple and Google are the sellers of record there [V].

**Apple and Google already act as merchant of record or deemed supplier in most jurisdictions**, remitting VAT themselves. This is why in-app purchase revenue needs no tax registration on your side while direct web sales do, and it is the single biggest hidden cost of moving to external payment links.

Tax obligations: EU VAT follows the **customer's** location with no registration threshold for non-EU suppliers, filed via One Stop Shop; business-to-business reverse charge requires a **validated** VAT ID, and an unvalidated one leaves you owing the VAT. US economic nexus follows *Wayfair* with per-state thresholds, and **digital-goods taxability varies by state**.

---

## Cross-platform entitlement architecture

### The state machine

**Entitled**: `active`, `in_trial`, `grace_period`, `active_until_expiry` (cancelled but unexpired).
**Not entitled**: `billing_retry` / `on_hold`, `paused`, `expired`, `revoked`, `superseded`.

Transitions worth modeling explicitly: payment failure branches to grace period (entitled) or billing retry (not entitled) depending on configuration; user cancellation moves to active-until-expiry, not expired; refunds and chargebacks move to revoked, **which may reverse**; upgrades supersede the old key rather than creating a parallel entitlement.

### Linking purchases to users

The anonymous-purchase-then-sign-up problem is real: a user can purchase before an account exists. Mint a stable anonymous identifier **before** the paywall, use it as the platform account token, and **merge** on sign-up.

**Handle transfer collision explicitly**: the same `originalTransactionId` appearing under a second user ID. Policies are transfer-to-newest, keep-with-original, or block and escalate. **The default of last-writer-wins enables deliberate account sharing.** Never key entitlement on device ID alone (lost on reinstall) or email alone (changes).

### Restore purchases

**App Review requires a restore mechanism for restorable purchases [V, guideline 3.1.1]**, and its absence is a common rejection. It must be reachable **without an account**, or a user who reinstalls and cannot log in has no path back to purchased content.

On Play, call `queryPurchasesAsync` at every app start, since purchases can complete out-of-app.

### Vendors

**RevenueCat** handles validation, cross-platform entitlement, and webhooks; free to $2,500 monthly tracked revenue then **1% of gross**, which is roughly 1.43% of net after a 30% commission [V2]. Its **`TEMPORARY_ENTITLEMENT_GRANT`** event grants short-term access during store outages [V] -- a **fail-open policy you are opting into**, which should be a deliberate choice. Adapty is comparable; **Superwall is paywall presentation, not an entitlement backend**.

Rolling your own is right for single-platform products (StoreKit 2 alone suffices for iOS-only), at revenue where 1% of gross exceeds engineering cost, or under regulatory constraints on third-party processors. The cost is not the happy path -- it is notification replay, out-of-order handling, refund reconciliation, upgrade-chain invalidation, and vendor deprecations, permanently.

### Clocks

Store timestamps as UTC epoch milliseconds. **Never compute entitlement from device time.** Do not recompute expiry by adding your own grace constant, since durations are platform-configurable. Month arithmetic is not 30 days; let the platform own renewal dates.

---

## Store policy: the payment rejection surface

**Apple 3.1.1** requires in-app purchase for unlocking features, and explicitly prohibits license keys, augmented-reality markers, QR codes, and cryptocurrency as alternative unlock mechanisms [V]. Purchased currencies **may not expire**, a restore mechanism is required, and loot boxes must disclose odds. **NFT ownership may not unlock app features** [V].

**Apple 3.1.2** requires subscriptions to deliver ongoing value, run a **minimum of seven days**, and **be available across all of the user's devices** [V]. Converting a paid app to subscriptions requires **existing users to retain the functionality they already paid for** [V].

**Apple 3.1.3** carves out reader apps, multiplatform services (provided the items are **also** available as in-app purchase), enterprise sales (consumer and family sales still require in-app purchase), and **person-to-person services limited to two individuals** -- one-to-few and one-to-many must use in-app purchase [V]. Physical goods and real-world services **must** use non-in-app-purchase methods; using in-app purchase there is itself a rejection [V].

**The US carve-out to anti-steering appears in five distinct guideline locations** [V]. Outside the US, anti-steering still binds, moderated by EU DMA terms.

**Disclosure** requirements bind both stores: price, billing period, auto-renewal, how to cancel, trial length, and the exact date and price of the first charge after a trial. **Burying trial-to-paid conversion terms is the most-cited dark-pattern rejection.**

**The FTC "click to cancel" rule was vacated [V].** The Eighth Circuit vacated the Negative Option Rule in its entirety on **2025-07-08**, days before compliance, on procedural grounds. An advance notice of proposed rulemaking followed on 2026-03-11. **It is not binding law** -- but enforcement continues under ROSCA and FTC Act section 5, and **state auto-renewal laws (notably California's) impose click-to-cancel duties independently**. Citing the federal rule as in force is wrong; concluding cancellation flows are unregulated is also wrong.

---

## Anti-pattern catalog

### Google Play
- Not acknowledging within 3 days; Google auto-refunds while your server shows the user as paid.
- Missing `.enableOneTimeProducts()` after the v8 migration, silently disabling cash payments.
- Granting entitlement on `PurchaseState.PENDING`.
- Using `orderId` as the primary key; promo-code purchases have none.
- Ignoring `linkedPurchaseToken`, producing double entitlement.
- Granting on `SUBSCRIPTION_PURCHASED` under `DEFERRED` replacement, granting the upgrade early.
- Revoking on `SUBSCRIPTION_CANCELED` rather than expiry.
- Treating notifications as truth without re-querying the Developer API.
- Hardcoding grace-period or account-hold durations.
- Never polling the Voided Purchases API, so chargebacks never revoke.
- Treating `ITEM_ALREADY_OWNED` as an error rather than querying and reconciling.
- No reconnection after `onBillingServiceDisconnected`, leaving billing silently dead.

### Apple
- Using `transactionId` rather than `originalTransactionId` as the subscription key.
- Switching on `RenewalState` without a default, on a struct with no exhaustiveness checking.
- Handling `INITIAL_BUY` as a notification type, silently dropping every first subscription.
- Confusing `RENEWAL_EXTENDED` with `RENEWAL_EXTENSION`.
- Treating milliunit prices as decimals.
- On-device-only validation, which cannot see a later refund even for honest clients.
- Accepting the payload from `VerificationResult` without checking `.verified`.
- Not attaching `Transaction.updates` at launch, losing Ask to Buy approvals.
- Calling `AppStore.sync()` automatically at launch, prompting for credentials.
- Posting V1 consumption fields to the V2 endpoint.
- Sending consumption data without consent.
- Not restoring entitlement on `REFUND_REVERSED`.
- Assuming `appAccountToken` works for family-shared transactions.
- Trusting client-computed introductory-offer eligibility.

### Stripe and web
- Verifying the signature against a re-serialized body, then disabling verification to "fix" it.
- Setting webhook tolerance to zero, disabling replay protection.
- Doing work before returning 2xx, stalling invoice finalization for 72 hours.
- Assuming webhook ordering.
- Expanding `invoice.subscription` after the `basil` version change.
- Revoking on `past_due` before retries complete.
- Treating `active` as proof all invoices are paid.
- Omitting `Idempotency-Key` on payment POSTs.
- Handling strong authentication only inline, stranding renewals.

### Cross-cutting
- A client-side `isSubscribed` boolean as the decision point.
- Pure event-driven entitlement with no reconciliation job.
- Computing entitlement from device clock.
- Recognizing gross store revenue as income rather than proceeds net of commission and VAT.
- No transfer-collision policy, enabling account sharing.
- Shipping premium content in the binary behind a client-side check.

---

## Schools of thought (preserve disagreement)

- **Roll your own vs a vendor.** The vendor case: validation, notification handling, upgrade chains, and reconciliation are a permanent tax, not a one-time build. The roll-your-own case: 1% of gross compounds, and you inherit a critical-path dependency and a data processor. The math turns somewhere around **$100-250K monthly tracked revenue**. For iOS-only, StoreKit 2 alone is genuinely sufficient. Be wary of opinions from people who have not priced the reconciliation work.
- **Fail open or fail closed during outages.** Near-consensus that the server is the source of truth; live disagreement on cache time-to-live and outage behavior. RevenueCat ships an explicit fail-open grant. Fail-closed protects revenue and punishes paying customers during **your own** outage. There is no universally right answer; the requirement is to choose deliberately and document it.
- **Merchant of record vs direct.** Crossover depends far more on jurisdictional spread than on volume: a small EU-heavy consumer seller may need one at any revenue, while a US-only business seller may never. **Stripe now sells both sides of this argument**, which is worth noting when reading Stripe's own material.
- **Whether to pursue external payment links.** Genuinely unresolved. US commission is 0% today, but certiorari was granted and a 15/10/5 rate is proposed; you take on PCI scope, tax liability, chargebacks, and materially worse conversion, and you must maintain both paths anyway.
- **Hard paywall vs freemium vs trial.** Reported 2026 figures show hard paywalls converting 10.7% at day 35 against freemium's 2.1% [V], but that measures conversion of installs reaching the paywall, not lifetime value, and hard paywalls suppress organic growth.
- **Trial length.** Long trials convert 42.5% against short trials' 25.5% [V], yet 46% of apps are moving to trials of four days or fewer [V]. The field is moving **against** the conversion data, defensible if you weight cash-cycle speed and trial-abuse reduction, but the tension is rarely acknowledged.

---

## What is NOT a platform-payments finding

- Pricing strategy, packaging, and paywall copy. Route to `product-leadership`.
- General cryptography or non-payment authentication. Route to `security`.
- General API contract design outside the entitlement surface. Route to `api-design`.
- Store submission mechanics unrelated to payments. Route to `platform-release`.
- Analytics event taxonomy for purchase events. Route to `web-analytics`.
- Generic retry and idempotency patterns off the payment path. Route to `distsys-runtime`.
- Speculation about undecided litigation as though it were settled law.
- Legal advice. This lens covers engineering and policy mechanics, not counsel.

---

## Severity calibration

Per `panel-contract.md`:

- **blocker**: client-side entitlement as the decision point; on-device-only receipt validation granting access; granting on Play `PENDING`; missing Play acknowledgment (auto-refund with retained access); webhook signature verification disabled or bypassed; accepting an unverified `VerificationResult` payload; premium content shipped in the binary behind a client check.
- **major**: `transactionId` or `orderId` as the durable key; `INITIAL_BUY` handled as a notification type; ignoring `linkedPurchaseToken`; no reconciliation job; `RenewalState` switch with no safe default; missing `.enableOneTimeProducts()` post-v8; revoking on cancellation rather than expiry; no restore mechanism; entitlement not restored on `REFUND_REVERSED`; no transfer-collision policy; webhook work before 2xx on `invoice.created`.
- **minor**: hardcoded grace durations; hardcoded legacy Apple domain; milliunit price mishandling in non-billing display paths; missing `obfuscatedAccountId`; no `Idempotency-Key`; `AppStore.sync()` at launch.
- **nit**: naming of entitlement states; log verbosity in webhook handlers.
- **insight**: structural -- "entitlement is derived independently in three places and they disagree on grace-period handling; centralize it"; "this is iOS-only, and the vendor dependency buys little that StoreKit 2 does not already provide"; "the fail-open cache policy during store outages is undocumented and should be a deliberate decision."

Confidence: high when the trigger is a concrete identifier, notification comparison, or missing acknowledgment call; medium when reasoned from architecture. **Lower confidence on any commission rate or policy claim, and say so** -- this domain's facts expire.

---

## Process for the platform-payments agent

1. **Identify the platforms and the surfaces**: StoreKit version, Play Billing version, Stripe, a vendor, or several.
2. **Find the durable key.** `originalTransactionId`, `purchaseToken`, Stripe subscription ID. A wrong key is usually the headline finding.
3. **Find the entitlement source of truth**, and what the client does when the server is unreachable.
4. **Walk the purchase path**: purchase, verification, server grant, acknowledgment or finish, and what happens if the process dies between any two steps.
5. **Walk the notification handler**: signature verification including chain validation, idempotency key, ordering assumptions, re-fetch versus blind transition, response timing.
6. **Walk the state machine**: is cancelled distinguished from expired, grace from retry, revoked from expired? Does refund reversal restore?
7. **Walk upgrades**: proration mode, linked-token invalidation, deferred-change timing.
8. **Walk restore and account linking**: restore reachable without an account, anonymous-purchase merge, transfer collision.
9. **Check reconciliation**: does a periodic job exist, and does it poll voided purchases?
10. **Check the policy surface**: restore present, disclosure complete, steering compliant for the target storefronts, correct product type for what is sold.
11. **Check version currency** against library deprecation deadlines.
12. **Route to other lenses**: pricing to `product-leadership`; cryptography to `security`; submission mechanics to `platform-release`; purchase analytics to `web-analytics`.
13. **Flag stale facts.** If the code encodes a commission rate, an anti-steering assumption, or a fee structure, note that these changed in 2025 and 2026 and require re-verification.
14. **Stay read-only.**
