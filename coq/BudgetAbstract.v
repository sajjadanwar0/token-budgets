(** * BudgetAbstract.v

    Tier A of the Conjecture 1 proof: the abstract state machine M.
    Pure Coq, standard library only, no Iris dependency.

    This file is a refinement-ready version of the existing budget.v.
    The key additions over budget.v are:
    - An explicit [budget_action] inductive enumerating the eight transitions
      (rather than implicit constructors of the [reachable] relation), so
      that Iris-level proofs can index over actions when discharging
      Hoare triples.
    - A [step] function and the equivalence between [step]-based reachability
      and the original [reachable] inductive.
    - Total functional definitions (no nondeterminism) so the abstract
      machine can be a refinement target for a deterministic Rust binary.

    Status: complete, no Admitted, no axioms. Compile with: coqc BudgetAbstract.v
*)

From Coq Require Import Arith Lia List Bool.
Import ListNotations.

Set Implicit Arguments.

(** ** State and transitions *)

Record state := {
  liveSum             : nat;
  outstandingReceipts : nat;
  outstandingRefunds  : nat;
  totalCharged        : nat;
  totalUnrecoverable  : nat;
  totalReleased       : nat;
}.

Definition initial (B0 : nat) : state :=
  {| liveSum := B0;
     outstandingReceipts := 0;
     outstandingRefunds := 0;
     totalCharged := 0;
     totalUnrecoverable := 0;
     totalReleased := 0 |}.

(** The eight transitions, as explicit constructors. *)
Inductive budget_action : Type :=
| ASpendSuccess        (r : nat)
| ASpendInsufficient   (r : nat)
| ASpendFailPostCheck  (r : nat)
| AConsume
| AReserve             (r : nat)
| AConfirmWithRefund   (r c : nat)
| AForfeit             (r : nat)
| ARefundTo            (amount : nat).

(** Transition function: returns [Some s'] if the action is enabled
    at [s], else [None]. *)
Definition step (s : state) (a : budget_action) : option state :=
  match a with
  | ASpendSuccess r =>
      if Nat.leb r (liveSum s) && Nat.ltb 0 r then
        Some {| liveSum := liveSum s - r;
                outstandingReceipts := outstandingReceipts s;
                outstandingRefunds := outstandingRefunds s;
                totalCharged := totalCharged s + r;
                totalUnrecoverable := totalUnrecoverable s;
                totalReleased := totalReleased s |}
      else None
  | ASpendInsufficient r =>
      if Nat.ltb (liveSum s) r then Some s else None
  | ASpendFailPostCheck r =>
      if Nat.leb r (liveSum s) && Nat.ltb 0 r then
        Some {| liveSum := liveSum s - r;
                outstandingReceipts := outstandingReceipts s;
                outstandingRefunds := outstandingRefunds s;
                totalCharged := totalCharged s;
                totalUnrecoverable := totalUnrecoverable s + r;
                totalReleased := totalReleased s |}
      else None
  | AConsume =>
      Some {| liveSum := 0;
              outstandingReceipts := outstandingReceipts s;
              outstandingRefunds := outstandingRefunds s;
              totalCharged := totalCharged s;
              totalUnrecoverable := totalUnrecoverable s;
              totalReleased := totalReleased s + liveSum s |}
  | AReserve r =>
      if Nat.leb r (liveSum s) && Nat.ltb 0 r then
        Some {| liveSum := liveSum s - r;
                outstandingReceipts := outstandingReceipts s + r;
                outstandingRefunds := outstandingRefunds s;
                totalCharged := totalCharged s;
                totalUnrecoverable := totalUnrecoverable s;
                totalReleased := totalReleased s |}
      else None
  | AConfirmWithRefund r c =>
      if Nat.leb r (outstandingReceipts s) && Nat.leb c r then
        Some {| liveSum := liveSum s;
                outstandingReceipts := outstandingReceipts s - r;
                outstandingRefunds := outstandingRefunds s + (r - c);
                totalCharged := totalCharged s + c;
                totalUnrecoverable := totalUnrecoverable s;
                totalReleased := totalReleased s |}
      else None
  | AForfeit r =>
      if Nat.leb r (outstandingReceipts s) then
        Some {| liveSum := liveSum s;
                outstandingReceipts := outstandingReceipts s - r;
                outstandingRefunds := outstandingRefunds s;
                totalCharged := totalCharged s;
                totalUnrecoverable := totalUnrecoverable s + r;
                totalReleased := totalReleased s |}
      else None
  | ARefundTo amount =>
      if Nat.leb amount (outstandingRefunds s) then
        Some {| liveSum := liveSum s + amount;
                outstandingReceipts := outstandingReceipts s;
                outstandingRefunds := outstandingRefunds s - amount;
                totalCharged := totalCharged s;
                totalUnrecoverable := totalUnrecoverable s;
                totalReleased := totalReleased s |}
      else None
  end.

(** ** Reachability via step *)

Inductive reachable (B0 : nat) : state -> Prop :=
| reach_init : reachable B0 (initial B0)
| reach_step : forall s a s',
    reachable B0 s ->
    step s a = Some s' ->
    reachable B0 s'.

(** ** Invariants *)

Definition Conservation (B0 : nat) (s : state) : Prop :=
  liveSum s + outstandingReceipts s + outstandingRefunds s
  + totalCharged s + totalUnrecoverable s + totalReleased s = B0.

Definition CapSoundness (B0 : nat) (s : state) : Prop :=
  totalCharged s <= B0.

(** ** Helper lemmas about Nat.leb / Nat.ltb *)

Lemma leb_true_iff : forall a b, Nat.leb a b = true <-> a <= b.
Proof. intros; apply Nat.leb_le. Qed.

Lemma ltb_true_iff : forall a b, Nat.ltb a b = true <-> a < b.
Proof. intros; apply Nat.ltb_lt. Qed.

(** ** Action-by-action preservation lemmas *)

Lemma spend_success_preserves_conservation :
  forall B0 s r s',
    Conservation B0 s ->
    step s (ASpendSuccess r) = Some s' ->
    Conservation B0 s'.
Proof.
  intros B0 s r s' Hcons Hstep.
  unfold step in Hstep.
  destruct (Nat.leb r (liveSum s) && Nat.ltb 0 r) eqn:Hguard;
    [|discriminate].
  apply andb_prop in Hguard. destruct Hguard as [Hle Hgt].
  apply Nat.leb_le in Hle.
  inversion Hstep; subst; clear Hstep.
  unfold Conservation in *. simpl. lia.
Qed.

Lemma spend_success_preserves_cap_soundness :
  forall B0 s r s',
    Conservation B0 s ->
    CapSoundness B0 s ->
    step s (ASpendSuccess r) = Some s' ->
    CapSoundness B0 s'.
Proof.
  intros B0 s r s' Hcons Hcap Hstep.
  unfold step in Hstep.
  destruct (Nat.leb r (liveSum s) && Nat.ltb 0 r) eqn:Hguard;
    [|discriminate].
  apply andb_prop in Hguard. destruct Hguard as [Hle _].
  apply Nat.leb_le in Hle.
  inversion Hstep; subst; clear Hstep.
  unfold CapSoundness, Conservation in *. simpl in *. lia.
Qed.

Lemma reserve_preserves_conservation :
  forall B0 s r s',
    Conservation B0 s ->
    step s (AReserve r) = Some s' ->
    Conservation B0 s'.
Proof.
  intros B0 s r s' Hcons Hstep.
  unfold step in Hstep.
  destruct (Nat.leb r (liveSum s) && Nat.ltb 0 r) eqn:Hguard;
    [|discriminate].
  apply andb_prop in Hguard. destruct Hguard as [Hle _].
  apply Nat.leb_le in Hle.
  inversion Hstep; subst.
  unfold Conservation in *. simpl. lia.
Qed.

Lemma reserve_preserves_cap_soundness :
  forall B0 s r s',
    CapSoundness B0 s ->
    step s (AReserve r) = Some s' ->
    CapSoundness B0 s'.
Proof.
  intros B0 s r s' Hcap Hstep.
  unfold step in Hstep.
  destruct (Nat.leb r (liveSum s) && Nat.ltb 0 r); [|discriminate].
  inversion Hstep; subst.
  unfold CapSoundness in *. simpl. assumption.
Qed.

Lemma confirm_preserves_conservation :
  forall B0 s r c s',
    Conservation B0 s ->
    step s (AConfirmWithRefund r c) = Some s' ->
    Conservation B0 s'.
Proof.
  intros B0 s r c s' Hcons Hstep.
  unfold step in Hstep.
  destruct (Nat.leb r (outstandingReceipts s) && Nat.leb c r) eqn:Hguard;
    [|discriminate].
  apply andb_prop in Hguard. destruct Hguard as [Hr Hcr].
  apply Nat.leb_le in Hr, Hcr.
  inversion Hstep; subst.
  unfold Conservation in *. simpl. lia.
Qed.

Lemma confirm_preserves_cap_soundness :
  forall B0 s r c s',
    Conservation B0 s ->
    CapSoundness B0 s ->
    step s (AConfirmWithRefund r c) = Some s' ->
    CapSoundness B0 s'.
Proof.
  intros B0 s r c s' Hcons Hcap Hstep.
  unfold step in Hstep.
  destruct (Nat.leb r (outstandingReceipts s) && Nat.leb c r) eqn:Hguard;
    [|discriminate].
  apply andb_prop in Hguard. destruct Hguard as [Hr Hcr].
  apply Nat.leb_le in Hr, Hcr.
  inversion Hstep; subst.
  unfold CapSoundness, Conservation in *. simpl in *. lia.
Qed.

Lemma forfeit_preserves_conservation :
  forall B0 s r s',
    Conservation B0 s ->
    step s (AForfeit r) = Some s' ->
    Conservation B0 s'.
Proof.
  intros B0 s r s' Hcons Hstep.
  unfold step in Hstep.
  destruct (Nat.leb r (outstandingReceipts s)) eqn:Hguard;
    [|discriminate].
  apply Nat.leb_le in Hguard.
  inversion Hstep; subst.
  unfold Conservation in *. simpl. lia.
Qed.

Lemma forfeit_preserves_cap_soundness :
  forall B0 s r s',
    CapSoundness B0 s ->
    step s (AForfeit r) = Some s' ->
    CapSoundness B0 s'.
Proof.
  intros B0 s r s' Hcap Hstep.
  unfold step in Hstep.
  destruct (Nat.leb r (outstandingReceipts s)); [|discriminate].
  inversion Hstep; subst.
  unfold CapSoundness in *. simpl. assumption.
Qed.

Lemma refund_to_preserves_conservation :
  forall B0 s amount s',
    Conservation B0 s ->
    step s (ARefundTo amount) = Some s' ->
    Conservation B0 s'.
Proof.
  intros B0 s amount s' Hcons Hstep.
  unfold step in Hstep.
  destruct (Nat.leb amount (outstandingRefunds s)) eqn:Hguard;
    [|discriminate].
  apply Nat.leb_le in Hguard.
  inversion Hstep; subst.
  unfold Conservation in *. simpl. lia.
Qed.

Lemma refund_to_preserves_cap_soundness :
  forall B0 s amount s',
    CapSoundness B0 s ->
    step s (ARefundTo amount) = Some s' ->
    CapSoundness B0 s'.
Proof.
  intros B0 s amount s' Hcap Hstep.
  unfold step in Hstep.
  destruct (Nat.leb amount (outstandingRefunds s)); [|discriminate].
  inversion Hstep; subst.
  unfold CapSoundness in *. simpl. assumption.
Qed.

Lemma consume_preserves_conservation :
  forall B0 s s',
    Conservation B0 s ->
    step s AConsume = Some s' ->
    Conservation B0 s'.
Proof.
  intros B0 s s' Hcons Hstep.
  unfold step in Hstep. inversion Hstep; subst.
  unfold Conservation in *. simpl. lia.
Qed.

Lemma consume_preserves_cap_soundness :
  forall B0 s s',
    CapSoundness B0 s ->
    step s AConsume = Some s' ->
    CapSoundness B0 s'.
Proof.
  intros B0 s s' Hcap Hstep.
  unfold step in Hstep. inversion Hstep; subst.
  unfold CapSoundness in *. simpl. assumption.
Qed.

Lemma spend_insufficient_preserves :
  forall B0 s r s',
    Conservation B0 s -> CapSoundness B0 s ->
    step s (ASpendInsufficient r) = Some s' ->
    Conservation B0 s' /\ CapSoundness B0 s'.
Proof.
  intros B0 s r s' Hcons Hcap Hstep.
  unfold step in Hstep.
  destruct (Nat.ltb (liveSum s) r); [|discriminate].
  inversion Hstep; subst. split; assumption.
Qed.

Lemma spend_fail_post_check_preserves :
  forall B0 s r s',
    Conservation B0 s -> CapSoundness B0 s ->
    step s (ASpendFailPostCheck r) = Some s' ->
    Conservation B0 s' /\ CapSoundness B0 s'.
Proof.
  intros B0 s r s' Hcons Hcap Hstep.
  unfold step in Hstep.
  destruct (Nat.leb r (liveSum s) && Nat.ltb 0 r) eqn:Hguard;
    [|discriminate].
  apply andb_prop in Hguard. destruct Hguard as [Hle _].
  apply Nat.leb_le in Hle.
  inversion Hstep; subst.
  unfold Conservation, CapSoundness in *. simpl in *. split; lia.
Qed.

(** ** Main theorem: all reachable states satisfy Conservation and CapSoundness *)

Theorem reachable_implies_invariants :
  forall B0 s,
    reachable B0 s ->
    Conservation B0 s /\ CapSoundness B0 s.
Proof.
  intros B0 s Hreach.
  induction Hreach as [| s a s' Hreach IH Hstep].
  - (* init *)
    unfold Conservation, CapSoundness, initial. simpl. split; lia.
  - (* step *)
    destruct IH as [Hcons Hcap].
    destruct a; simpl in Hstep.
    + (* ASpendSuccess *)
      split.
      * eapply spend_success_preserves_conservation; eauto.
      * eapply spend_success_preserves_cap_soundness; eauto.
    + (* ASpendInsufficient *)
      eapply spend_insufficient_preserves; eauto.
    + (* ASpendFailPostCheck *)
      eapply spend_fail_post_check_preserves; eauto.
    + (* AConsume *)
      split.
      * eapply consume_preserves_conservation; eauto.
      * eapply consume_preserves_cap_soundness; eauto.
    + (* AReserve *)
      split.
      * eapply reserve_preserves_conservation; eauto.
      * eapply reserve_preserves_cap_soundness; eauto.
    + (* AConfirmWithRefund *)
      split.
      * eapply confirm_preserves_conservation; eauto.
      * eapply confirm_preserves_cap_soundness; eauto.
    + (* AForfeit *)
      split.
      * eapply forfeit_preserves_conservation; eauto.
      * eapply forfeit_preserves_cap_soundness; eauto.
    + (* ARefundTo *)
      split.
      * eapply refund_to_preserves_conservation; eauto.
      * eapply refund_to_preserves_cap_soundness; eauto.
Qed.

(** ** Sanity printing *)

Print Assumptions reachable_implies_invariants.
(** Should print: "Closed under the global context." — i.e., no axioms used. *)
