_ZN3app6script12_GLOBAL__N_19Color_newEP9lua_Statei:
  0x3c54a0  55                   push rbp
  0x3c54a1  4157                 push r15
  0x3c54a3  4156                 push r14
  0x3c54a5  4155                 push r13
  0x3c54a7  4154                 push r12
  0x3c54a9  53                   push rbx
  0x3c54aa  4883ec48             sub rsp, 0x48
  0x3c54ae  4189d7               mov r15d, edx
  0x3c54b1  4989f6               mov r14, rsi
  0x3c54b4  4889fb               mov rbx, rdi
  0x3c54b7  64488b042528000000   mov rax, fs:[0x28]
  0x3c54c0  4889442440           mov [rsp+0x40], rax
  0x3c54c5  c70700000000         mov dword ptr [rdi], 0
  0x3c54cb  488d157aa6b700       lea rdx, [0xf3fb4c]  ; "app::Color"
  0x3c54d2  4889f7               mov rdi, rsi
  0x3c54d5  4489fe               mov esi, r15d
  0x3c54d8  e8239e1600           call 0x52f300  ; _Z14luaL_testudataP9lua_StateiPKc
  0x3c54dd  4885c0               test rax, rax
  0x3c54e0  742f                 je 0x3c5511
loc_3c54e2:
  0x3c54e2  64488b0c2528000000   mov rcx, fs:[0x28]
  0x3c54eb  483b4c2440           cmp rcx, [rsp+0x40]
  0x3c54f0  0f8599060000         jne 0x3c5b8f
loc_3c54f6:
  0x3c54f6  488b4820             mov rcx, [rax+0x20]
  0x3c54fa  48894b20             mov [rbx+0x20], rcx
  0x3c54fe  0f1000               movups xmm0, [rax]
  0x3c5501  0f104810             movups xmm1, [rax+0x10]
  0x3c5505  0f114b10             movups [rbx+0x10], xmm1
  0x3c5509  0f1103               movups [rbx], xmm0
  0x3c550c  e9dd020000           jmp 0x3c57ee
loc_3c5511:
  0x3c5511  4c89f7               mov rdi, r14
  0x3c5514  4489fe               mov esi, r15d
  0x3c5517  e8548d1700           call 0x53e270  ; _Z8lua_typeP9lua_Statei
  0x3c551c  83f805               cmp eax, 5
  0x3c551f  754a                 jne 0x3c556b
loc_3c5521:
  0x3c5521  488d15c120ba00       lea rdx, [0xf675e9]  ; "r"
  0x3c5528  4c89f7               mov rdi, r14
  0x3c552b  4489fe               mov esi, r15d
  0x3c552e  e8dda11700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c5533  85c0                 test eax, eax
  0x3c5535  0f84c4000000         je 0x3c55ff
loc_3c553b:
  0x3c553b  488d1503cfb900       lea rdx, [0xf62445]  ; "g"
  0x3c5542  4c89f7               mov rdi, r14
  0x3c5545  4489fe               mov esi, r15d
  0x3c5548  e8c3a11700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c554d  488d156fc2ba00       lea rdx, [0xf717c3]
  0x3c5554  4c89f7               mov rdi, r14
  0x3c5557  4489fe               mov esi, r15d
  0x3c555a  e8b1a11700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c555f  488d152f2cb800       lea rdx, [0xf48195]
  0x3c5566  e9e6000000           jmp 0x3c5651
loc_3c556b:
  0x3c556b  4c89f7               mov rdi, r14
  0x3c556e  4489fe               mov esi, r15d
  0x3c5571  e8fa8c1700           call 0x53e270  ; _Z8lua_typeP9lua_Statei
  0x3c5576  83f8ff               cmp eax, 0xffffffff
  0x3c5579  0f845b020000         je 0x3c57da
loc_3c557f:
  0x3c557f  4c89f7               mov rdi, r14
  0x3c5582  4489fe               mov esi, r15d
  0x3c5585  e8368e1700           call 0x53e3c0  ; _Z13lua_isintegerP9lua_Statei
  0x3c558a  85c0                 test eax, eax
  0x3c558c  0f845c010000         je 0x3c56ee
loc_3c5592:
  0x3c5592  4585ff               test r15d, r15d
  0x3c5595  7817                 js 0x3c55ae
loc_3c5597:
  0x3c5597  418d6f01             lea ebp, [r15+1]
  0x3c559b  4c89f7               mov rdi, r14
  0x3c559e  89ee                 mov esi, ebp
  0x3c55a0  e8cb8c1700           call 0x53e270  ; _Z8lua_typeP9lua_Statei
  0x3c55a5  83f8ff               cmp eax, 0xffffffff
  0x3c55a8  0f854d010000         jne 0x3c56fb
loc_3c55ae:
  0x3c55ae  4c89f7               mov rdi, r14
  0x3c55b1  4489fe               mov esi, r15d
  0x3c55b4  31d2                 xor edx, edx
  0x3c55b6  e8f5941700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c55bb  4989c6               mov r14, rax
  0x3c55be  e86d7fd8ff           call 0x14d530  ; _ZN3app28app_get_current_pixel_formatEv
  0x3c55c3  83f802               cmp eax, 2
  0x3c55c6  0f8446020000         je 0x3c5812
loc_3c55cc:
  0x3c55cc  83f801               cmp eax, 1
  0x3c55cf  0f8428020000         je 0x3c57fd
loc_3c55d5:
  0x3c55d5  85c0                 test eax, eax
  0x3c55d7  0f85fd010000         jne 0x3c57da
loc_3c55dd:
  0x3c55dd  4c89f1               mov rcx, r14
  0x3c55e0  0fb6f1               movzx esi, cl
  0x3c55e3  0fb6d5               movzx edx, ch
  0x3c55e6  89c8                 mov eax, ecx
  0x3c55e8  4d89f0               mov r8, r14
  0x3c55eb  c1e810               shr eax, 0x10
  0x3c55ee  0fb6c8               movzx ecx, al
  0x3c55f1  41c1e818             shr r8d, 0x18
  0x3c55f5  488d7c2418           lea rdi, [rsp+0x18]
  0x3c55fa  e9bc010000           jmp 0x3c57bb
loc_3c55ff:
  0x3c55ff  4c89f7               mov rdi, r14
  0x3c5602  befeffffff           mov esi, 0xfffffffe
  0x3c5607  e854881700           call 0x53de60  ; _Z10lua_settopP9lua_Statei
  0x3c560c  488d152cc6b800       lea rdx, [0xf51c3f]  ; "red"
  0x3c5613  4c89f7               mov rdi, r14
  0x3c5616  4489fe               mov esi, r15d
  0x3c5619  e8f2a01700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c561e  85c0                 test eax, eax
  0x3c5620  0f842b010000         je 0x3c5751
loc_3c5626:
  0x3c5626  488d15da23b700       lea rdx, [0xf37a07]  ; "green"
  0x3c562d  4c89f7               mov rdi, r14
  0x3c5630  4489fe               mov esi, r15d
  0x3c5633  e8d8a01700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c5638  488d15e023b700       lea rdx, [0xf37a1f]  ; "blue"
  0x3c563f  4c89f7               mov rdi, r14
  0x3c5642  4489fe               mov esi, r15d
  0x3c5645  e8c6a01700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c564a  488d15471bb700       lea rdx, [0xf37198]  ; "alpha"
loc_3c5651:
  0x3c5651  4c89f7               mov rdi, r14
  0x3c5654  4489fe               mov esi, r15d
  0x3c5657  e8b4a01700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c565c  85c0                 test eax, eax
  0x3c565e  7414                 je 0x3c5674
loc_3c5660:
  0x3c5660  4c89f7               mov rdi, r14
  0x3c5663  beffffffff           mov esi, 0xffffffff
  0x3c5668  31d2                 xor edx, edx
  0x3c566a  e841941700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c566f  4989c7               mov r15, rax
  0x3c5672  eb06                 jmp 0x3c567a
loc_3c5674:
  0x3c5674  41bfff000000         mov r15d, 0xff
loc_3c567a:
  0x3c567a  4c89f7               mov rdi, r14
  0x3c567d  befcffffff           mov esi, 0xfffffffc
  0x3c5682  31d2                 xor edx, edx
  0x3c5684  e827941700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c5689  4989c4               mov r12, rax
  0x3c568c  4c89f7               mov rdi, r14
  0x3c568f  befdffffff           mov esi, 0xfffffffd
  0x3c5694  31d2                 xor edx, edx
  0x3c5696  e815941700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c569b  4989c5               mov r13, rax
  0x3c569e  4c89f7               mov rdi, r14
  0x3c56a1  befeffffff           mov esi, 0xfffffffe
  0x3c56a6  31d2                 xor edx, edx
  0x3c56a8  e803941700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c56ad  488d7c2418           lea rdi, [rsp+0x18]
  0x3c56b2  4489e6               mov esi, r12d
  0x3c56b5  4489ea               mov edx, r13d
  0x3c56b8  89c1                 mov ecx, eax
  0x3c56ba  4589f8               mov r8d, r15d
  0x3c56bd  e8fe4edaff           call 0x16a5c0  ; _ZN3app5Color7fromRgbEiiii
loc_3c56c2:
  0x3c56c2  488b442438           mov rax, [rsp+0x38]
  0x3c56c7  48894320             mov [rbx+0x20], rax
  0x3c56cb  0f10442418           movups xmm0, [rsp+0x18]
  0x3c56d0  0f104c2428           movups xmm1, [rsp+0x28]
  0x3c56d5  0f114b10             movups [rbx+0x10], xmm1
  0x3c56d9  0f1103               movups [rbx], xmm0
  0x3c56dc  4c89f7               mov rdi, r14
  0x3c56df  befbffffff           mov esi, 0xfffffffb
  0x3c56e4  e877871700           call 0x53de60  ; _Z10lua_settopP9lua_Statei
  0x3c56e9  e9ec000000           jmp 0x3c57da
loc_3c56ee:
  0x3c56ee  4585ff               test r15d, r15d
  0x3c56f1  0f88e3000000         js 0x3c57da
loc_3c56f7:
  0x3c56f7  418d6f01             lea ebp, [r15+1]
loc_3c56fb:
  0x3c56fb  4c89f7               mov rdi, r14
  0x3c56fe  4489fe               mov esi, r15d
  0x3c5701  31d2                 xor edx, edx
  0x3c5703  e8a8931700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c5708  4989c4               mov r12, rax
  0x3c570b  4c89f7               mov rdi, r14
  0x3c570e  89ee                 mov esi, ebp
  0x3c5710  31d2                 xor edx, edx
  0x3c5712  e899931700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c5717  4989c5               mov r13, rax
  0x3c571a  418d7702             lea esi, [r15+2]
  0x3c571e  4c89f7               mov rdi, r14
  0x3c5721  31d2                 xor edx, edx
  0x3c5723  e888931700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c5728  4889c5               mov rbp, rax
  0x3c572b  4183c703             add r15d, 3
  0x3c572f  4c89f7               mov rdi, r14
  0x3c5732  4489fe               mov esi, r15d
  0x3c5735  e8368b1700           call 0x53e270  ; _Z8lua_typeP9lua_Statei
  0x3c573a  83f8ff               cmp eax, 0xffffffff
  0x3c573d  7469                 je 0x3c57a8
loc_3c573f:
  0x3c573f  4c89f7               mov rdi, r14
  0x3c5742  4489fe               mov esi, r15d
  0x3c5745  31d2                 xor edx, edx
  0x3c5747  e864931700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c574c  4989c0               mov r8, rax
  0x3c574f  eb5d                 jmp 0x3c57ae
loc_3c5751:
  0x3c5751  4c89f7               mov rdi, r14
  0x3c5754  befeffffff           mov esi, 0xfffffffe
  0x3c5759  e802871700           call 0x53de60  ; _Z10lua_settopP9lua_Statei
  0x3c575e  488d153cf6b600       lea rdx, [0xf34da1]
  0x3c5765  4c89f7               mov rdi, r14
  0x3c5768  4489fe               mov esi, r15d
  0x3c576b  e8a09f1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c5770  85c0                 test eax, eax
  0x3c5772  0f84a9000000         je 0x3c5821
loc_3c5778:
  0x3c5778  488d158625b800       lea rdx, [0xf47d05]  ; "s"
  0x3c577f  4c89f7               mov rdi, r14
  0x3c5782  4489fe               mov esi, r15d
  0x3c5785  e8869f1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c578a  488d153dcab900       lea rdx, [0xf621ce]  ; "h"
  0x3c5791  4c89f7               mov rdi, r14
  0x3c5794  4489fe               mov esi, r15d
  0x3c5797  e8749f1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c579c  488d15f229b800       lea rdx, [0xf48195]
  0x3c57a3  e9cb000000           jmp 0x3c5873
loc_3c57a8:
  0x3c57a8  41b8ff000000         mov r8d, 0xff
loc_3c57ae:
  0x3c57ae  488d7c2418           lea rdi, [rsp+0x18]
  0x3c57b3  4489e6               mov esi, r12d
  0x3c57b6  4489ea               mov edx, r13d
  0x3c57b9  89e9                 mov ecx, ebp
loc_3c57bb:
  0x3c57bb  e8004edaff           call 0x16a5c0  ; _ZN3app5Color7fromRgbEiiii
loc_3c57c0:
  0x3c57c0  488b442438           mov rax, [rsp+0x38]
  0x3c57c5  48894320             mov [rbx+0x20], rax
  0x3c57c9  0f10442418           movups xmm0, [rsp+0x18]
  0x3c57ce  0f104c2428           movups xmm1, [rsp+0x28]
  0x3c57d3  0f114b10             movups [rbx+0x10], xmm1
  0x3c57d7  0f1103               movups [rbx], xmm0
loc_3c57da:
  0x3c57da  64488b042528000000   mov rax, fs:[0x28]
  0x3c57e3  483b442440           cmp rax, [rsp+0x40]
  0x3c57e8  0f85a1030000         jne 0x3c5b8f
loc_3c57ee:
  0x3c57ee  4883c448             add rsp, 0x48
  0x3c57f2  5b                   pop rbx
  0x3c57f3  415c                 pop r12
  0x3c57f5  415d                 pop r13
  0x3c57f7  415e                 pop r14
  0x3c57f9  415f                 pop r15
  0x3c57fb  5d                   pop rbp
  0x3c57fc  c3                   ret
loc_3c57fd:
  0x3c57fd  4c89f0               mov rax, r14
  0x3c5800  0fb6f0               movzx esi, al
  0x3c5803  0fb6d4               movzx edx, ah
  0x3c5806  488d7c2418           lea rdi, [rsp+0x18]
  0x3c580b  e8104edaff           call 0x16a620  ; _ZN3app5Color8fromGrayEii
  0x3c5810  ebae                 jmp 0x3c57c0
loc_3c5812:
  0x3c5812  488d7c2418           lea rdi, [rsp+0x18]
  0x3c5817  4489f6               mov esi, r14d
  0x3c581a  e8114edaff           call 0x16a630  ; _ZN3app5Color9fromIndexEi
  0x3c581f  eb9f                 jmp 0x3c57c0
loc_3c5821:
  0x3c5821  4c89f7               mov rdi, r14
  0x3c5824  befeffffff           mov esi, 0xfffffffe
  0x3c5829  e832861700           call 0x53de60  ; _Z10lua_settopP9lua_Statei
  0x3c582e  488d159d37bb00       lea rdx, [0xf78fd2]  ; "value"
  0x3c5835  4c89f7               mov rdi, r14
  0x3c5838  4489fe               mov esi, r15d
  0x3c583b  e8d09e1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c5840  85c0                 test eax, eax
  0x3c5842  0f84ae000000         je 0x3c58f6
loc_3c5848:
  0x3c5848  488d15a1d9b700       lea rdx, [0xf431f0]  ; "saturation"
  0x3c584f  4c89f7               mov rdi, r14
  0x3c5852  4489fe               mov esi, r15d
  0x3c5855  e8b69e1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c585a  488d157ad9b700       lea rdx, [0xf431db]  ; "hue"
  0x3c5861  4c89f7               mov rdi, r14
  0x3c5864  4489fe               mov esi, r15d
  0x3c5867  e8a49e1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c586c  488d152519b700       lea rdx, [0xf37198]  ; "alpha"
loc_3c5873:
  0x3c5873  4c89f7               mov rdi, r14
  0x3c5876  4489fe               mov esi, r15d
  0x3c5879  e8929e1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c587e  85c0                 test eax, eax
  0x3c5880  7414                 je 0x3c5896
loc_3c5882:
  0x3c5882  4c89f7               mov rdi, r14
  0x3c5885  beffffffff           mov esi, 0xffffffff
  0x3c588a  31d2                 xor edx, edx
  0x3c588c  e81f921700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c5891  4989c7               mov r15, rax
  0x3c5894  eb06                 jmp 0x3c589c
loc_3c5896:
  0x3c5896  41bfff000000         mov r15d, 0xff
loc_3c589c:
  0x3c589c  4c89f7               mov rdi, r14
  0x3c589f  befeffffff           mov esi, 0xfffffffe
  0x3c58a4  31d2                 xor edx, edx
  0x3c58a6  e815911700           call 0x53e9c0  ; _Z13lua_tonumberxP9lua_StateiPi
  0x3c58ab  f20f11442410         movsd [rsp+0x10], xmm0
  0x3c58b1  4c89f7               mov rdi, r14
  0x3c58b4  befdffffff           mov esi, 0xfffffffd
  0x3c58b9  31d2                 xor edx, edx
  0x3c58bb  e800911700           call 0x53e9c0  ; _Z13lua_tonumberxP9lua_StateiPi
  0x3c58c0  f20f11442408         movsd [rsp+8], xmm0
  0x3c58c6  4c89f7               mov rdi, r14
  0x3c58c9  befcffffff           mov esi, 0xfffffffc
  0x3c58ce  31d2                 xor edx, edx
  0x3c58d0  e8eb901700           call 0x53e9c0  ; _Z13lua_tonumberxP9lua_StateiPi
  0x3c58d5  0f28d0               movaps xmm2, xmm0
  0x3c58d8  488d7c2418           lea rdi, [rsp+0x18]
  0x3c58dd  f20f10442410         movsd xmm0, [rsp+0x10]
  0x3c58e3  f20f104c2408         movsd xmm1, [rsp+8]
  0x3c58e9  4489fe               mov esi, r15d
  0x3c58ec  e8ef4cdaff           call 0x16a5e0  ; _ZN3app5Color7fromHsvEdddi
  0x3c58f1  e9ccfdffff           jmp 0x3c56c2
loc_3c58f6:
  0x3c58f6  4c89f7               mov rdi, r14
  0x3c58f9  befeffffff           mov esi, 0xfffffffe
  0x3c58fe  e85d851700           call 0x53de60  ; _Z10lua_settopP9lua_Statei
  0x3c5903  488d1542a2b800       lea rdx, [0xf4fb4c]  ; "l"
  0x3c590a  4c89f7               mov rdi, r14
  0x3c590d  4489fe               mov esi, r15d
  0x3c5910  e8fb9d1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c5915  85c0                 test eax, eax
  0x3c5917  742d                 je 0x3c5946
loc_3c5919:
  0x3c5919  488d15e523b800       lea rdx, [0xf47d05]  ; "s"
  0x3c5920  4c89f7               mov rdi, r14
  0x3c5923  4489fe               mov esi, r15d
  0x3c5926  e8e59d1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c592b  488d159cc8b900       lea rdx, [0xf621ce]  ; "h"
  0x3c5932  4c89f7               mov rdi, r14
  0x3c5935  4489fe               mov esi, r15d
  0x3c5938  e8d39d1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c593d  488d155128b800       lea rdx, [0xf48195]
  0x3c5944  eb52                 jmp 0x3c5998
loc_3c5946:
  0x3c5946  4c89f7               mov rdi, r14
  0x3c5949  befeffffff           mov esi, 0xfffffffe
  0x3c594e  e80d851700           call 0x53de60  ; _Z10lua_settopP9lua_Statei
  0x3c5953  488d153a49b700       lea rdx, [0xf3a294]  ; "lightness"
  0x3c595a  4c89f7               mov rdi, r14
  0x3c595d  4489fe               mov esi, r15d
  0x3c5960  e8ab9d1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c5965  85c0                 test eax, eax
  0x3c5967  0f84ae000000         je 0x3c5a1b
loc_3c596d:
  0x3c596d  488d157cd8b700       lea rdx, [0xf431f0]  ; "saturation"
  0x3c5974  4c89f7               mov rdi, r14
  0x3c5977  4489fe               mov esi, r15d
  0x3c597a  e8919d1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c597f  488d1555d8b700       lea rdx, [0xf431db]  ; "hue"
  0x3c5986  4c89f7               mov rdi, r14
  0x3c5989  4489fe               mov esi, r15d
  0x3c598c  e87f9d1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c5991  488d150018b700       lea rdx, [0xf37198]  ; "alpha"
loc_3c5998:
  0x3c5998  4c89f7               mov rdi, r14
  0x3c599b  4489fe               mov esi, r15d
  0x3c599e  e86d9d1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c59a3  85c0                 test eax, eax
  0x3c59a5  7414                 je 0x3c59bb
loc_3c59a7:
  0x3c59a7  4c89f7               mov rdi, r14
  0x3c59aa  beffffffff           mov esi, 0xffffffff
  0x3c59af  31d2                 xor edx, edx
  0x3c59b1  e8fa901700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c59b6  4989c7               mov r15, rax
  0x3c59b9  eb06                 jmp 0x3c59c1
loc_3c59bb:
  0x3c59bb  41bfff000000         mov r15d, 0xff
loc_3c59c1:
  0x3c59c1  4c89f7               mov rdi, r14
  0x3c59c4  befeffffff           mov esi, 0xfffffffe
  0x3c59c9  31d2                 xor edx, edx
  0x3c59cb  e8f08f1700           call 0x53e9c0  ; _Z13lua_tonumberxP9lua_StateiPi
  0x3c59d0  f20f11442410         movsd [rsp+0x10], xmm0
  0x3c59d6  4c89f7               mov rdi, r14
  0x3c59d9  befdffffff           mov esi, 0xfffffffd
  0x3c59de  31d2                 xor edx, edx
  0x3c59e0  e8db8f1700           call 0x53e9c0  ; _Z13lua_tonumberxP9lua_StateiPi
  0x3c59e5  f20f11442408         movsd [rsp+8], xmm0
  0x3c59eb  4c89f7               mov rdi, r14
  0x3c59ee  befcffffff           mov esi, 0xfffffffc
  0x3c59f3  31d2                 xor edx, edx
  0x3c59f5  e8c68f1700           call 0x53e9c0  ; _Z13lua_tonumberxP9lua_StateiPi
  0x3c59fa  0f28d0               movaps xmm2, xmm0
  0x3c59fd  488d7c2418           lea rdi, [rsp+0x18]
  0x3c5a02  f20f10442410         movsd xmm0, [rsp+0x10]
  0x3c5a08  f20f104c2408         movsd xmm1, [rsp+8]
  0x3c5a0e  4489fe               mov esi, r15d
  0x3c5a11  e8ea4bdaff           call 0x16a600  ; _ZN3app5Color7fromHslEdddi
  0x3c5a16  e9a7fcffff           jmp 0x3c56c2
loc_3c5a1b:
  0x3c5a1b  4c89f7               mov rdi, r14
  0x3c5a1e  befeffffff           mov esi, 0xfffffffe
  0x3c5a23  e838841700           call 0x53de60  ; _Z10lua_settopP9lua_Statei
  0x3c5a28  488d159aa6b900       lea rdx, [0xf600c9]  ; "gray"
  0x3c5a2f  4c89f7               mov rdi, r14
  0x3c5a32  4489fe               mov esi, r15d
  0x3c5a35  e8d69c1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c5a3a  85c0                 test eax, eax
  0x3c5a3c  742a                 je 0x3c5a68
loc_3c5a3e:
  0x3c5a3e  488d155317b700       lea rdx, [0xf37198]  ; "alpha"
  0x3c5a45  4c89f7               mov rdi, r14
  0x3c5a48  4489fe               mov esi, r15d
  0x3c5a4b  e8c09c1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c5a50  85c0                 test eax, eax
  0x3c5a52  7457                 je 0x3c5aab
loc_3c5a54:
  0x3c5a54  4c89f7               mov rdi, r14
  0x3c5a57  beffffffff           mov esi, 0xffffffff
  0x3c5a5c  31d2                 xor edx, edx
  0x3c5a5e  e84d901700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c5a63  4989c7               mov r15, rax
  0x3c5a66  eb49                 jmp 0x3c5ab1
loc_3c5a68:
  0x3c5a68  4c89f7               mov rdi, r14
  0x3c5a6b  befeffffff           mov esi, 0xfffffffe
  0x3c5a70  e8eb831700           call 0x53de60  ; _Z10lua_settopP9lua_Statei
  0x3c5a75  488d15dbabb900       lea rdx, [0xf60657]  ; "index"
  0x3c5a7c  4c89f7               mov rdi, r14
  0x3c5a7f  4489fe               mov esi, r15d
  0x3c5a82  e8899c1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c5a87  4c89f7               mov rdi, r14
  0x3c5a8a  85c0                 test eax, eax
  0x3c5a8c  746f                 je 0x3c5afd
loc_3c5a8e:
  0x3c5a8e  beffffffff           mov esi, 0xffffffff
  0x3c5a93  31d2                 xor edx, edx
  0x3c5a95  e816901700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c5a9a  488d7c2418           lea rdi, [rsp+0x18]
  0x3c5a9f  89c6                 mov esi, eax
  0x3c5aa1  e88a4bdaff           call 0x16a630  ; _ZN3app5Color9fromIndexEi
  0x3c5aa6  e98d000000           jmp 0x3c5b38
loc_3c5aab:
  0x3c5aab  41bfff000000         mov r15d, 0xff
loc_3c5ab1:
  0x3c5ab1  4c89f7               mov rdi, r14
  0x3c5ab4  befeffffff           mov esi, 0xfffffffe
  0x3c5ab9  31d2                 xor edx, edx
  0x3c5abb  e8008f1700           call 0x53e9c0  ; _Z13lua_tonumberxP9lua_StateiPi
  0x3c5ac0  f20f2cf0             cvttsd2si esi, xmm0
  0x3c5ac4  488d7c2418           lea rdi, [rsp+0x18]
  0x3c5ac9  4489fa               mov edx, r15d
  0x3c5acc  e84f4bdaff           call 0x16a620  ; _ZN3app5Color8fromGrayEii
  0x3c5ad1  488b442438           mov rax, [rsp+0x38]
  0x3c5ad6  48894320             mov [rbx+0x20], rax
  0x3c5ada  0f10442418           movups xmm0, [rsp+0x18]
  0x3c5adf  0f104c2428           movups xmm1, [rsp+0x28]
  0x3c5ae4  0f114b10             movups [rbx+0x10], xmm1
  0x3c5ae8  0f1103               movups [rbx], xmm0
  0x3c5aeb  4c89f7               mov rdi, r14
  0x3c5aee  befdffffff           mov esi, 0xfffffffd
  0x3c5af3  e868831700           call 0x53de60  ; _Z10lua_settopP9lua_Statei
  0x3c5af8  e9ddfcffff           jmp 0x3c57da
loc_3c5afd:
  0x3c5afd  befeffffff           mov esi, 0xfffffffe
  0x3c5b02  e859831700           call 0x53de60  ; _Z10lua_settopP9lua_Statei
  0x3c5b07  488d156c42b800       lea rdx, [0xf49d7a]  ; "tile"
  0x3c5b0e  4c89f7               mov rdi, r14
  0x3c5b11  4489fe               mov esi, r15d
  0x3c5b14  e8f79b1700           call 0x53f710  ; _Z12lua_getfieldP9lua_StateiPKc
  0x3c5b19  85c0                 test eax, eax
  0x3c5b1b  7447                 je 0x3c5b64
loc_3c5b1d:
  0x3c5b1d  4c89f7               mov rdi, r14
  0x3c5b20  beffffffff           mov esi, 0xffffffff
  0x3c5b25  31d2                 xor edx, edx
  0x3c5b27  e8848f1700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c5b2c  488d7c2418           lea rdi, [rsp+0x18]
  0x3c5b31  89c6                 mov esi, eax
  0x3c5b33  e8084bdaff           call 0x16a640  ; _ZN3app5Color8fromTileEj
loc_3c5b38:
  0x3c5b38  488b442438           mov rax, [rsp+0x38]
  0x3c5b3d  48894320             mov [rbx+0x20], rax
  0x3c5b41  0f10442418           movups xmm0, [rsp+0x18]
  0x3c5b46  0f104c2428           movups xmm1, [rsp+0x28]
  0x3c5b4b  0f114b10             movups [rbx+0x10], xmm1
  0x3c5b4f  0f1103               movups [rbx], xmm0
  0x3c5b52  4c89f7               mov rdi, r14
  0x3c5b55  befeffffff           mov esi, 0xfffffffe
  0x3c5b5a  e801831700           call 0x53de60  ; _Z10lua_settopP9lua_Statei
  0x3c5b5f  e976fcffff           jmp 0x3c57da
loc_3c5b64:
  0x3c5b64  64488b042528000000   mov rax, fs:[0x28]
  0x3c5b6d  483b442440           cmp rax, [rsp+0x40]
  0x3c5b72  751b                 jne 0x3c5b8f
loc_3c5b74:
  0x3c5b74  4c89f7               mov rdi, r14
  0x3c5b77  befeffffff           mov esi, 0xfffffffe
  0x3c5b7c  4883c448             add rsp, 0x48
  0x3c5b80  5b                   pop rbx
  0x3c5b81  415c                 pop r12
  0x3c5b83  415d                 pop r13
  0x3c5b85  415e                 pop r14
  0x3c5b87  415f                 pop r15
  0x3c5b89  5d                   pop rbp
  0x3c5b8a  e9d1821700           jmp 0x53de60  ; _Z10lua_settopP9lua_Statei
loc_3c5b8f:
  0x3c5b8f  e84c0ad8ff           call 0x1465e0  ; __stack_chk_fail@plt
