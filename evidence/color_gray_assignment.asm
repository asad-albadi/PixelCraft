_ZN3app6script12_GLOBAL__N_114Color_set_grayEP9lua_State:
  0x3c6920  4156                 push r14
  0x3c6922  53                   push rbx
  0x3c6923  4883ec38             sub rsp, 0x38
  0x3c6927  4889fb               mov rbx, rdi
  0x3c692a  64488b042528000000   mov rax, fs:[0x28]
  0x3c6933  4889442430           mov [rsp+0x30], rax
  0x3c6938  488d150d92b700       lea rdx, [0xf3fb4c]  ; "app::Color"
  0x3c693f  be01000000           mov esi, 1
  0x3c6944  e8378a1600           call 0x52f380  ; _Z15luaL_checkudataP9lua_StateiPKc
  0x3c6949  4989c6               mov r14, rax
  0x3c694c  4889df               mov rdi, rbx
  0x3c694f  be02000000           mov esi, 2
  0x3c6954  31d2                 xor edx, edx
  0x3c6956  e855811700           call 0x53eab0  ; _Z14lua_tointegerxP9lua_StateiPi
  0x3c695b  4889c3               mov rbx, rax
  0x3c695e  4c89f7               mov rdi, r14
  0x3c6961  e83a49daff           call 0x16b2a0  ; _ZNK3app5Color8getAlphaEv
  0x3c6966  488d7c2408           lea rdi, [rsp+8]
  0x3c696b  89de                 mov esi, ebx
  0x3c696d  89c2                 mov edx, eax
  0x3c696f  e8ac3cdaff           call 0x16a620  ; _ZN3app5Color8fromGrayEii
  0x3c6974  488b442428           mov rax, [rsp+0x28]
  0x3c6979  49894620             mov [r14+0x20], rax
  0x3c697d  0f10442408           movups xmm0, [rsp+8]
  0x3c6982  0f104c2418           movups xmm1, [rsp+0x18]
  0x3c6987  410f114e10           movups [r14+0x10], xmm1
  0x3c698c  410f1106             movups [r14], xmm0
  0x3c6990  64488b042528000000   mov rax, fs:[0x28]
  0x3c6999  483b442430           cmp rax, [rsp+0x30]
  0x3c699e  750a                 jne 0x3c69aa
loc_3c69a0:
  0x3c69a0  31c0                 xor eax, eax
  0x3c69a2  4883c438             add rsp, 0x38
  0x3c69a6  5b                   pop rbx
  0x3c69a7  415e                 pop r14
  0x3c69a9  c3                   ret
loc_3c69aa:
  0x3c69aa  e831fcd7ff           call 0x1465e0  ; __stack_chk_fail@plt

_ZN3app5Color8fromGrayEii:
  0x16a620  4889f8               mov rax, rdi
  0x16a623  c70704000000         mov dword ptr [rdi], 4
  0x16a629  897708               mov [rdi+8], esi
  0x16a62c  89570c               mov [rdi+0xc], edx
  0x16a62f  c3                   ret

_ZNK3app5Color7getGrayEv:
  0x16c2b0  55                   push rbp
  0x16c2b1  4156                 push r14
  0x16c2b3  53                   push rbx
  0x16c2b4  4883ec30             sub rsp, 0x30
  0x16c2b8  64488b042528000000   mov rax, fs:[0x28]
  0x16c2c1  4889442428           mov [rsp+0x28], rax
  0x16c2c6  8b07                 mov eax, [rdi]
  0x16c2c8  4883f805             cmp rax, 5
  0x16c2cc  0f87fb000000         ja 0x16c3cd
loc_16c2d2:
  0x16c2d2  488d0d2f57e100       lea rcx, [0xf81a08]
  0x16c2d9  48631481             movsxd rdx, [rcx+rax*4]
  0x16c2dd  4801ca               add rdx, rcx
  0x16c2e0  ffe2                 jmp rdx
loc_16c2e2:
  0x16c2e2  8b4710               mov eax, [rdi+0x10]
  0x16c2e5  488b4f08             mov rcx, [rdi+8]
  0x16c2e9  48890c24             mov [rsp], rcx
  0x16c2ed  e99f000000           jmp 0x16c391
loc_16c2f2:
  0x16c2f2  8b4708               mov eax, [rdi+8]
  0x16c2f5  64488b0c2528000000   mov rcx, fs:[0x28]
  0x16c2fe  483b4c2428           cmp rcx, [rsp+0x28]
  0x16c303  0f84bb000000         je 0x16c3c4
loc_16c309:
  0x16c309  e9d4000000           jmp 0x16c3e2
loc_16c30e:
  0x16c30e  4889fb               mov rbx, rdi
  0x16c311  e82aebffff           call 0x16ae40  ; _ZNK3app5Color6getRedEv
  0x16c316  89c5                 mov ebp, eax
  0x16c318  4889df               mov rdi, rbx
  0x16c31b  e880ecffff           call 0x16afa0  ; _ZNK3app5Color8getGreenEv
  0x16c320  4189c6               mov r14d, eax
  0x16c323  4889df               mov rdi, rbx
  0x16c326  e8f5edffff           call 0x16b120  ; _ZNK3app5Color7getBlueEv
  0x16c32b  892c24               mov [rsp], ebp
  0x16c32e  4489742404           mov [rsp+4], r14d
  0x16c333  eb5c                 jmp 0x16c391
loc_16c335:
  0x16c335  f20f104718           movsd xmm0, [rdi+0x18]
  0x16c33a  eb6c                 jmp 0x16c3a8
loc_16c33c:
  0x16c33c  48635f08             movsxd rbx, [rdi+8]
  0x16c340  31c0                 xor eax, eax
  0x16c342  4885db               test rbx, rbx
  0x16c345  786d                 js 0x16c3b4
loc_16c347:
  0x16c347  e8fc610700           call 0x1e2548  ; _ZN3app19get_current_paletteEv
  0x16c34c  4889c1               mov rcx, rax
  0x16c34f  31c0                 xor eax, eax
  0x16c351  488b5128             mov rdx, [rcx+0x28]
  0x16c355  482b5120             sub rdx, [rcx+0x20]
  0x16c359  48c1ea02             shr rdx, 2
  0x16c35d  39d3                 cmp ebx, edx
  0x16c35f  7d53                 jge 0x16c3b4
loc_16c361:
  0x16c361  e8e2610700           call 0x1e2548  ; _ZN3app19get_current_paletteEv
  0x16c366  488b4820             mov rcx, [rax+0x20]
  0x16c36a  488b5028             mov rdx, [rax+0x28]
  0x16c36e  4829ca               sub rdx, rcx
  0x16c371  48c1ea02             shr rdx, 2
  0x16c375  31c0                 xor eax, eax
  0x16c377  39d3                 cmp ebx, edx
  0x16c379  7d03                 jge 0x16c37e
loc_16c37b:
  0x16c37b  8b0499               mov eax, [rcx+rbx*4]
loc_16c37e:
  0x16c37e  0fb6c8               movzx ecx, al
  0x16c381  0fb6d4               movzx edx, ah
  0x16c384  c1e810               shr eax, 0x10
  0x16c387  0fb6c0               movzx eax, al
  0x16c38a  890c24               mov [rsp], ecx
  0x16c38d  89542404             mov [rsp+4], edx
loc_16c391:
  0x16c391  89442408             mov [rsp+8], eax
  0x16c395  488d7c2410           lea rdi, [rsp+0x10]
  0x16c39a  4889e6               mov rsi, rsp
  0x16c39d  e80e505200           call 0x6913b0  ; _ZN3gfx3HslC2ERKNS_3RgbE
  0x16c3a2  f20f10442420         movsd xmm0, [rsp+0x20]
loc_16c3a8:
  0x16c3a8  f20f590548f0e000     mulsd xmm0, [0xf7b3f8]
  0x16c3b0  f20f2cc0             cvttsd2si eax, xmm0
loc_16c3b4:
  0x16c3b4  64488b0c2528000000   mov rcx, fs:[0x28]
  0x16c3bd  483b4c2428           cmp rcx, [rsp+0x28]
  0x16c3c2  751e                 jne 0x16c3e2
loc_16c3c4:
  0x16c3c4  4883c430             add rsp, 0x30
  0x16c3c8  5b                   pop rbx
  0x16c3c9  415e                 pop r14
  0x16c3cb  5d                   pop rbp
  0x16c3cc  c3                   ret
loc_16c3cd:
  0x16c3cd  b8ffffffff           mov eax, 0xffffffff
  0x16c3d2  64488b0c2528000000   mov rcx, fs:[0x28]
  0x16c3db  483b4c2428           cmp rcx, [rsp+0x28]
  0x16c3e0  74e2                 je 0x16c3c4
loc_16c3e2:
  0x16c3e2  e8f9a1fdff           call 0x1465e0  ; __stack_chk_fail@plt
