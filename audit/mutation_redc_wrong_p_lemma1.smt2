; Lemma 1 — REDC low-word invariant (basis of the wrong_p equivalence):
; for all a, b in [0, P): lo32(prod + m*P) = 0, where prod = a*b (exact 64-bit)
; and m = lo32(lo32(prod) * NEG_INV). This is the invariant that makes the
; vmlal-constant's low bits unobservable in the REDC output. Negated; unsat.
(set-option :produce-models true)
(set-logic QF_BV)
(declare-const a (_ BitVec 32))
(declare-const b (_ BitVec 32))
(assert (bvult a #x78000001))
(assert (bvult b #x78000001))
(define-fun prod () (_ BitVec 64) (bvmul ((_ zero_extend 32) a) ((_ zero_extend 32) b)))
(define-fun x () (_ BitVec 32) ((_ extract 31 0) prod))
(define-fun m () (_ BitVec 32) ((_ extract 31 0) (bvmul ((_ zero_extend 32) x) ((_ zero_extend 32) #x77ffffff))))
(define-fun mp () (_ BitVec 64) (bvadd prod (bvmul ((_ zero_extend 32) m) ((_ zero_extend 32) #x78000001))))
(assert (not (= ((_ extract 31 0) mp) #x00000000)))
(check-sat)
(exit)
