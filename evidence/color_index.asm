_ZN3app6script12_GLOBAL__N_115Color_set_indexEP9lua_State:
  0x3c6860  4156                 push r14
  0x3c6862  53                   push rbx
  0x3c6863  4883ec38             sub rsp, 0x38
  0x3c6867  4889fb               mov rbx, rdi
  0x3c686a  64488b042528000000   mov rax, fs:[0x28]
  0x3c6873  4889442430           mov [rsp+0x30], rax
  0x3c6878  488d15cd92b700       lea rdx, [0xf3fb4c]  ; "app::Color"
  0x3c687f  be01000000           mov esi, 1
  0x3c6884  e8f78a1600           call 0x52f380  ; _Z15luaL_checkudataP9lua_StateiPKc
  0x3c6889  4989c6               mov r14, rax
  0x3c688c  4889df               mov rdi, rbx
  0x3c688f  be02000000           mov esi, 2
  0x3c6894  31d2                 xor edx, edx
  0x3c6896  e815821700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c689b  488d7c2408           lea rdi, [rsp+8]
  0x3c68a0  89c6                 mov esi, eax
  0x3c68a2  e8893ddaff           call 0x16a630  ; _ZN3app5Color9fromIndexEi
  0x3c68a7  488b442428           mov rax, [rsp+0x28]
  0x3c68ac  49894620             mov [r14+0x20], rax
  0x3c68b0  0f10442408           movups xmm0, [rsp+8]
  0x3c68b5  0f104c2418           movups xmm1, [rsp+0x18]
  0x3c68ba  410f114e10           movups [r14+0x10], xmm1
  0x3c68bf  410f1106             movups [r14], xmm0
  0x3c68c3  64488b042528000000   mov rax, fs:[0x28]
  0x3c68cc  483b442430           cmp rax, [rsp+0x30]
  0x3c68d1  750a                 jne 0x3c68dd
loc_3c68d3:
  0x3c68d3  31c0                 xor eax, eax
  0x3c68d5  4883c438             add rsp, 0x38
  0x3c68d9  5b                   pop rbx
  0x3c68da  415e                 pop r14
  0x3c68dc  c3                   ret
loc_3c68dd:
  0x3c68dd  e8fefcd7ff           call 0x1465e0  ; __stack_chk_fail@plt

_ZNK3app5Color8getIndexEv:
  0x16cbb0  55                   push rbp
  0x16cbb1  4157                 push r15
  0x16cbb3  4156                 push r14
  0x16cbb5  4155                 push r13
  0x16cbb7  4154                 push r12
  0x16cbb9  53                   push rbx
  0x16cbba  50                   push rax
  0x16cbbb  8b07                 mov eax, [rdi]
  0x16cbbd  4883f806             cmp rax, 6
  0x16cbc1  775a                 ja 0x16cc1d
loc_16cbc3:
  0x16cbc3  4889fb               mov rbx, rdi
  0x16cbc6  488d0dfb4ee100       lea rcx, [0xf81ac8]
  0x16cbcd  48631481             movsxd rdx, [rcx+rax*4]
  0x16cbd1  4801ca               add rdx, rcx
  0x16cbd4  ffe2                 jmp rdx
loc_16cbd6:
  0x16cbd6  e86d590700           call 0x1e2548  ; _ZN3app19get_current_paletteEv
  0x16cbdb  4989c6               mov r14, rax
  0x16cbde  4889df               mov rdi, rbx
  0x16cbe1  e85ae2ffff           call 0x16ae40  ; _ZNK3app5Color6getRedEv
  0x16cbe6  89c5                 mov ebp, eax
  0x16cbe8  4889df               mov rdi, rbx
  0x16cbeb  e8b0e3ffff           call 0x16afa0  ; _ZNK3app5Color8getGreenEv
  0x16cbf0  4189c7               mov r15d, eax
  0x16cbf3  4889df               mov rdi, rbx
  0x16cbf6  e825e5ffff           call 0x16b120  ; _ZNK3app5Color7getBlueEv
  0x16cbfb  448b03               mov r8d, [rbx]
  0x16cbfe  4983f806             cmp r8, 6
  0x16cc02  7744                 ja 0x16cc48
loc_16cc04:
  0x16cc04  488d0dd94ee100       lea rcx, [0xf81ae4]
  0x16cc0b  4a631481             movsxd rdx, [rcx+r8*4]
  0x16cc0f  4801ca               add rdx, rcx
  0x16cc12  ffe2                 jmp rdx
loc_16cc14:
  0x16cc14  448b4320             mov r8d, [rbx+0x20]
  0x16cc18  e990000000           jmp 0x16ccad
loc_16cc1d:
  0x16cc1d  b8ffffffff           mov eax, 0xffffffff
  0x16cc22  e99f000000           jmp 0x16ccc6
loc_16cc27:
  0x16cc27  8b4308               mov eax, [rbx+8]
  0x16cc2a  e997000000           jmp 0x16ccc6
loc_16cc2f:
  0x16cc2f  b8ffffff1f           mov eax, 0x1fffffff
  0x16cc34  234308               and eax, [rbx+8]
  0x16cc37  e98a000000           jmp 0x16ccc6
loc_16cc3c:
  0x16cc3c  448b430c             mov r8d, [rbx+0xc]
  0x16cc40  eb6b                 jmp 0x16ccad
loc_16cc42:
  0x16cc42  448b4314             mov r8d, [rbx+0x14]
  0x16cc46  eb65                 jmp 0x16ccad
loc_16cc48:
  0x16cc48  41b8ffffffff         mov r8d, 0xffffffff
  0x16cc4e  eb5d                 jmp 0x16ccad
loc_16cc50:
  0x16cc50  4c636308             movsxd r12, [rbx+8]
  0x16cc54  4531c0               xor r8d, r8d
  0x16cc57  4d85e4               test r12, r12
  0x16cc5a  7851                 js 0x16ccad
loc_16cc5c:
  0x16cc5c  4189c5               mov r13d, eax
  0x16cc5f  e8e4580700           call 0x1e2548  ; _ZN3app19get_current_paletteEv
  0x16cc64  488b4828             mov rcx, [rax+0x28]
  0x16cc68  482b4820             sub rcx, [rax+0x20]
  0x16cc6c  48c1e902             shr rcx, 2
  0x16cc70  4139cc               cmp r12d, ecx
  0x16cc73  7d32                 jge 0x16cca7
loc_16cc75:
  0x16cc75  e8ce580700           call 0x1e2548  ; _ZN3app19get_current_paletteEv
  0x16cc7a  488b4820             mov rcx, [rax+0x20]
  0x16cc7e  488b4028             mov rax, [rax+0x28]
  0x16cc82  4829c8               sub rax, rcx
  0x16cc85  48c1e802             shr rax, 2
  0x16cc89  4139c4               cmp r12d, eax
  0x16cc8c  4489e8               mov eax, r13d
  0x16cc8f  41b800000000         mov r8d, 0
  0x16cc95  7d16                 jge 0x16ccad
loc_16cc97:
  0x16cc97  460fb644a103         movzx r8d, byte ptr [rcx+r12*4+3]
  0x16cc9d  eb0e                 jmp 0x16ccad
loc_16cc9f:
  0x16cc9f  41b8ff000000         mov r8d, 0xff
  0x16cca5  eb06                 jmp 0x16ccad
loc_16cca7:
  0x16cca7  4489e8               mov eax, r13d
  0x16ccaa  4531c0               xor r8d, r8d
loc_16ccad:
  0x16ccad  4c89f7               mov rdi, r14
  0x16ccb0  89ee                 mov esi, ebp
  0x16ccb2  4489fa               mov edx, r15d
  0x16ccb5  89c1                 mov ecx, eax
  0x16ccb7  41b9ffffffff         mov r9d, 0xffffffff
  0x16ccbd  e86ee74900           call 0x60b430  ; _ZNK3doc7Palette14findExactMatchEiiiii
  0x16ccc2  85c0                 test eax, eax
  0x16ccc4  780f                 js 0x16ccd5
loc_16ccc6:
  0x16ccc6  4883c408             add rsp, 8
  0x16ccca  5b                   pop rbx
  0x16cccb  415c                 pop r12
  0x16cccd  415d                 pop r13
  0x16cccf  415e                 pop r14
  0x16ccd1  415f                 pop r15
  0x16ccd3  5d                   pop rbp
  0x16ccd4  c3                   ret
loc_16ccd5:
  0x16ccd5  e86e580700           call 0x1e2548  ; _ZN3app19get_current_paletteEv
  0x16ccda  4989c6               mov r14, rax
  0x16ccdd  4889df               mov rdi, rbx
  0x16cce0  e85be1ffff           call 0x16ae40  ; _ZNK3app5Color6getRedEv
  0x16cce5  89c5                 mov ebp, eax
  0x16cce7  4889df               mov rdi, rbx
  0x16ccea  e8b1e2ffff           call 0x16afa0  ; _ZNK3app5Color8getGreenEv
  0x16ccef  4189c7               mov r15d, eax
  0x16ccf2  4889df               mov rdi, rbx
  0x16ccf5  e826e4ffff           call 0x16b120  ; _ZNK3app5Color7getBlueEv
  0x16ccfa  448b03               mov r8d, [rbx]
  0x16ccfd  4983f806             cmp r8, 6
  0x16cd01  7722                 ja 0x16cd25
loc_16cd03:
  0x16cd03  488d0df64de100       lea rcx, [0xf81b00]
  0x16cd0a  4a631481             movsxd rdx, [rcx+r8*4]
  0x16cd0e  4801ca               add rdx, rcx
  0x16cd11  ffe2                 jmp rdx
loc_16cd13:
  0x16cd13  448b4320             mov r8d, [rbx+0x20]
  0x16cd17  eb6f                 jmp 0x16cd88
loc_16cd19:
  0x16cd19  448b430c             mov r8d, [rbx+0xc]
  0x16cd1d  eb69                 jmp 0x16cd88
loc_16cd1f:
  0x16cd1f  448b4314             mov r8d, [rbx+0x14]
  0x16cd23  eb63                 jmp 0x16cd88
loc_16cd25:
  0x16cd25  41b8ffffffff         mov r8d, 0xffffffff
  0x16cd2b  eb5b                 jmp 0x16cd88
loc_16cd2d:
  0x16cd2d  48635b08             movsxd rbx, [rbx+8]
  0x16cd31  4531c0               xor r8d, r8d
  0x16cd34  4885db               test rbx, rbx
  0x16cd37  784f                 js 0x16cd88
loc_16cd39:
  0x16cd39  4189c4               mov r12d, eax
  0x16cd3c  e807580700           call 0x1e2548  ; _ZN3app19get_current_paletteEv
  0x16cd41  488b4828             mov rcx, [rax+0x28]
  0x16cd45  482b4820             sub rcx, [rax+0x20]
  0x16cd49  48c1e902             shr rcx, 2
  0x16cd4d  39cb                 cmp ebx, ecx
  0x16cd4f  7d31                 jge 0x16cd82
loc_16cd51:
  0x16cd51  e8f2570700           call 0x1e2548  ; _ZN3app19get_current_paletteEv
  0x16cd56  488b4820             mov rcx, [rax+0x20]
  0x16cd5a  488b4028             mov rax, [rax+0x28]
  0x16cd5e  4829c8               sub rax, rcx
  0x16cd61  48c1e802             shr rax, 2
  0x16cd65  39c3                 cmp ebx, eax
  0x16cd67  4489e0               mov eax, r12d
  0x16cd6a  41b800000000         mov r8d, 0
  0x16cd70  7d16                 jge 0x16cd88
loc_16cd72:
  0x16cd72  440fb6449903         movzx r8d, byte ptr [rcx+rbx*4+3]
  0x16cd78  eb0e                 jmp 0x16cd88
loc_16cd7a:
  0x16cd7a  41b8ff000000         mov r8d, 0xff
  0x16cd80  eb06                 jmp 0x16cd88
loc_16cd82:
  0x16cd82  4489e0               mov eax, r12d
  0x16cd85  4531c0               xor r8d, r8d
loc_16cd88:
  0x16cd88  4c89f7               mov rdi, r14
  0x16cd8b  89ee                 mov esi, ebp
  0x16cd8d  4489fa               mov edx, r15d
  0x16cd90  89c1                 mov ecx, eax
  0x16cd92  4531c9               xor r9d, r9d
  0x16cd95  4883c408             add rsp, 8
  0x16cd99  5b                   pop rbx
  0x16cd9a  415c                 pop r12
  0x16cd9c  415d                 pop r13
  0x16cd9e  415e                 pop r14
  0x16cda0  415f                 pop r15
  0x16cda2  5d                   pop rbp
  0x16cda3  e9d8e94900           jmp 0x60b780  ; _ZNK3doc7Palette11findBestfitEiiiii
