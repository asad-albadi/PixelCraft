
aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

000000000050e0a0 <_ZN3app6script12_GLOBAL__N_115Cel_set_opacityEP9lua_State>:
  50e0a0:	41 57                	push   %r15
  50e0a2:	41 56                	push   %r14
  50e0a4:	53                   	push   %rbx
  50e0a5:	48 83 ec 40          	sub    $0x40,%rsp
  50e0a9:	48 89 fb             	mov    %rdi,%rbx
  50e0ac:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  50e0b3:	00 00 
  50e0b5:	48 89 44 24 38       	mov    %rax,0x38(%rsp)
  50e0ba:	48 8d 15 b7 74 a3 00 	lea    0xa374b7(%rip),%rdx        # f45578 <_ZL16gUnaryActiveEdge+0x11e14>
  50e0c1:	be 01 00 00 00       	mov    $0x1,%esi
  50e0c6:	e8 35 12 02 00       	call   52f300 <_Z14luaL_testudataP9lua_StateiPKc>
  50e0cb:	48 85 c0             	test   %rax,%rax
  50e0ce:	74 0f                	je     50e0df <_ZN3app6script12_GLOBAL__N_115Cel_set_opacityEP9lua_State+0x3f>
  50e0d0:	8b 38                	mov    (%rax),%edi
  50e0d2:	e8 39 97 0f 00       	call   607810 <_ZN3doc10get_objectEj>
  50e0d7:	48 89 c6             	mov    %rax,%rsi
  50e0da:	48 85 c0             	test   %rax,%rax
  50e0dd:	75 02                	jne    50e0e1 <_ZN3app6script12_GLOBAL__N_115Cel_set_opacityEP9lua_State+0x41>
  50e0df:	31 f6                	xor    %esi,%esi
  50e0e1:	48 89 df             	mov    %rbx,%rdi
  50e0e4:	e8 a7 b0 ec ff       	call   3d9190 <_ZN3app6script12check_docobjIN3doc3CelEEEPT_P9lua_StateS5_>
  50e0e9:	49 89 c6             	mov    %rax,%r14
  50e0ec:	48 89 c7             	mov    %rax,%rdi
  50e0ef:	e8 6c 6d 0e 00       	call   5f4e60 <_ZNK3doc3Cel6spriteEv>
  50e0f4:	4c 8d 7c 24 28       	lea    0x28(%rsp),%r15
  50e0f9:	4c 89 7c 24 18       	mov    %r15,0x18(%rsp)
  50e0fe:	48 b9 54 72 61 6e 73 	movabs $0x746361736e617254,%rcx
  50e105:	61 63 74 
  50e108:	48 89 4c 24 28       	mov    %rcx,0x28(%rsp)
  50e10d:	c7 44 24 2f 74 69 6f 	movl   $0x6e6f6974,0x2f(%rsp)
  50e114:	6e 
  50e115:	c6 44 24 33 00       	movb   $0x0,0x33(%rsp)
  50e11a:	48 c7 44 24 20 0b 00 	movq   $0xb,0x20(%rsp)
  50e121:	00 00 
  50e123:	48 8b 48 70          	mov    0x70(%rax),%rcx
  50e127:	48 8b 91 90 00 00 00 	mov    0x90(%rcx),%rdx
  50e12e:	48 89 e7             	mov    %rsp,%rdi
  50e131:	4c 8d 44 24 18       	lea    0x18(%rsp),%r8
  50e136:	be 01 00 00 00       	mov    $0x1,%esi
  50e13b:	45 31 c9             	xor    %r9d,%r9d
  50e13e:	e8 4d 94 c6 ff       	call   177590 <_ZN3app2TxC2ENS0_10LockActionEPNS_7ContextEPNS_3DocERKNSt7__cxx1112basic_stringIcSt11char_traitsIcESaIcEEENS_12ModificationE>
  50e143:	48 8b 7c 24 18       	mov    0x18(%rsp),%rdi
  50e148:	4c 39 ff             	cmp    %r15,%rdi
  50e14b:	74 0d                	je     50e15a <_ZN3app6script12_GLOBAL__N_115Cel_set_opacityEP9lua_State+0xba>
  50e14d:	48 8b 74 24 28       	mov    0x28(%rsp),%rsi
  50e152:	48 ff c6             	inc    %rsi
  50e155:	e8 86 80 c3 ff       	call   1461e0 <_ZdlPvm@plt>
  50e15a:	bf 20 00 00 00       	mov    $0x20,%edi
  50e15f:	e8 3c 95 c3 ff       	call   1476a0 <_Znwm@plt>
  50e164:	49 89 c7             	mov    %rax,%r15
  50e167:	48 89 df             	mov    %rbx,%rdi
  50e16a:	be 02 00 00 00       	mov    $0x2,%esi
  50e16f:	31 d2                	xor    %edx,%edx
  50e171:	e8 3a 09 03 00       	call   53eab0 <_Z14lua_tointegerxP9lua_StateiPi>
  50e176:	4c 89 ff             	mov    %r15,%rdi
  50e179:	4c 89 f6             	mov    %r14,%rsi
  50e17c:	89 c2                	mov    %eax,%edx
  50e17e:	e8 0d 6e f0 ff       	call   414f90 <_ZN3app3cmd13SetCelOpacityC1EPN3doc3CelEi>
  50e183:	48 8b 7c 24 08       	mov    0x8(%rsp),%rdi
  50e188:	4c 89 fe             	mov    %r15,%rsi
  50e18b:	e8 90 93 d1 ff       	call   227520 <_ZN3app11Transaction7executeEPNS_3CmdE>
  50e190:	80 7c 24 14 01       	cmpb   $0x1,0x14(%rsp)
  50e195:	75 2c                	jne    50e1c3 <_ZN3app6script12_GLOBAL__N_115Cel_set_opacityEP9lua_State+0x123>
  50e197:	48 8b 7c 24 08       	mov    0x8(%rsp),%rdi
  50e19c:	e8 8f 92 d1 ff       	call   227430 <_ZN3app11Transaction6commitEv>
  50e1a1:	80 7c 24 14 01       	cmpb   $0x1,0x14(%rsp)
  50e1a6:	75 1b                	jne    50e1c3 <_ZN3app6script12_GLOBAL__N_115Cel_set_opacityEP9lua_State+0x123>
  50e1a8:	48 8b 3c 24          	mov    (%rsp),%rdi
  50e1ac:	31 f6                	xor    %esi,%esi
  50e1ae:	e8 9d bf c8 ff       	call   19a150 <_ZN3app3Doc14setTransactionEPNS_11TransactionE>
  50e1b3:	48 8b 7c 24 08       	mov    0x8(%rsp),%rdi
  50e1b8:	48 85 ff             	test   %rdi,%rdi
  50e1bb:	74 06                	je     50e1c3 <_ZN3app6script12_GLOBAL__N_115Cel_set_opacityEP9lua_State+0x123>
  50e1bd:	48 8b 07             	mov    (%rdi),%rax
  50e1c0:	ff 50 08             	call   *0x8(%rax)
  50e1c3:	8b 74 24 10          	mov    0x10(%rsp),%esi
  50e1c7:	85 f6                	test   %esi,%esi
  50e1c9:	74 09                	je     50e1d4 <_ZN3app6script12_GLOBAL__N_115Cel_set_opacityEP9lua_State+0x134>
  50e1cb:	48 8b 3c 24          	mov    (%rsp),%rdi
  50e1cf:	e8 4c bf c8 ff       	call   19a120 <_ZN3app3Doc6unlockEN4base6RWLock10LockResultE>
  50e1d4:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  50e1db:	00 00 
  50e1dd:	48 3b 44 24 38       	cmp    0x38(%rsp),%rax
  50e1e2:	75 65                	jne    50e249 <_ZN3app6script12_GLOBAL__N_115Cel_set_opacityEP9lua_State+0x1a9>
  50e1e4:	31 c0                	xor    %eax,%eax
  50e1e6:	48 83 c4 40          	add    $0x40,%rsp
  50e1ea:	5b                   	pop    %rbx
  50e1eb:	41 5e                	pop    %r14
  50e1ed:	41 5f                	pop    %r15
  50e1ef:	c3                   	ret
  50e1f0:	48 89 c7             	mov    %rax,%rdi
  50e1f3:	e8 e8 ce c3 ff       	call   14b0e0 <__clang_call_terminate>
  50e1f8:	48 89 c3             	mov    %rax,%rbx
  50e1fb:	48 8b 7c 24 18       	mov    0x18(%rsp),%rdi
  50e200:	4c 39 ff             	cmp    %r15,%rdi
  50e203:	74 2c                	je     50e231 <_ZN3app6script12_GLOBAL__N_115Cel_set_opacityEP9lua_State+0x191>
  50e205:	48 8b 74 24 28       	mov    0x28(%rsp),%rsi
  50e20a:	48 ff c6             	inc    %rsi
  50e20d:	e8                   	.byte 0xe8
