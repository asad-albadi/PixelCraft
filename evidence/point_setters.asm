
aseprite:     file format elf64-x86-64


Disassembly of section .text:

00000000003e5050 <_ZN3app6script12_GLOBAL__N_111Point_set_xEP9lua_State>:
  3e5050:	41 56                	push   %r14
  3e5052:	53                   	push   %rbx
  3e5053:	50                   	push   %rax
  3e5054:	48 89 fb             	mov    %rdi,%rbx
  3e5057:	48 8d 15 5d b9 b5 00 	lea    0xb5b95d(%rip),%rdx        # f409bb <_ZL16gUnaryActiveEdge+0xd257>
  3e505e:	be 01 00 00 00       	mov    $0x1,%esi
  3e5063:	e8 18 a3 14 00       	call   52f380 <_Z15luaL_checkudataP9lua_StateiPKc>
  3e5068:	49 89 c6             	mov    %rax,%r14
  3e506b:	48 89 df             	mov    %rbx,%rdi
  3e506e:	be 02 00 00 00       	mov    $0x2,%esi
  3e5073:	31 d2                	xor    %edx,%edx
  3e5075:	e8 36 9a 15 00       	call   53eab0 <_Z14lua_tointegerxP9lua_StateiPi>
  3e507a:	41 89 06             	mov    %eax,(%r14)
  3e507d:	31 c0                	xor    %eax,%eax
  3e507f:	48 83 c4 08          	add    $0x8,%rsp
  3e5083:	5b                   	pop    %rbx
  3e5084:	41 5e                	pop    %r14
  3e5086:	c3                   	ret
  3e5087:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  3e508e:	00 00

00000000003e5090 <_ZN3app6script12_GLOBAL__N_111Point_get_yEP9lua_State>:
  3e5090:	53                   	push   %rbx
  3e5091:	48 89 fb             	mov    %rdi,%rbx
  3e5094:	48 8d 15 20 b9 b5 00 	lea    0xb5b920(%rip),%rdx        # f409bb <_ZL16gUnaryActiveEdge+0xd257>
  3e509b:	be 01 00 00 00       	mov    $0x1,%esi
  3e50a0:	e8 db a2 14 00       	call   52f380 <_Z15luaL_checkudataP9lua_StateiPKc>
  3e50a5:	48 63 70 04          	movslq 0x4(%rax),%rsi
  3e50a9:	48 89 df             	mov    %rbx,%rdi
  3e50ac:	e8 5f a1 15 00       	call   53f210 <_Z15lua_pushintegerP9lua_Statex>
  3e50b1:	b8 01 00 00 00       	mov    $0x1,%eax
  3e50b6:	5b                   	pop    %rbx
  3e50b7:	c3                   	ret
  3e50b8:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  3e50bf:	00

00000000003e50c0 <_ZN3app6script12_GLOBAL__N_111Point_set_yEP9lua_State>:
  3e50c0:	41 56                	push   %r14
  3e50c2:	53                   	push   %rbx
  3e50c3:	50                   	push   %rax
  3e50c4:	48 89 fb             	mov    %rdi,%rbx
  3e50c7:	48 8d 15 ed b8 b5 00 	lea    0xb5b8ed(%rip),%rdx        # f409bb <_ZL16gUnaryActiveEdge+0xd257>
  3e50ce:	be 01 00 00 00       	mov    $0x1,%esi
  3e50d3:	e8 a8 a2 14 00       	call   52f380 <_Z15luaL_checkudataP9lua_StateiPKc>
  3e50d8:	49 89 c6             	mov    %rax,%r14
  3e50db:	48 89 df             	mov    %rbx,%rdi
  3e50de:	be 02 00 00 00       	mov    $0x2,%esi
  3e50e3:	31 d2                	xor    %edx,%edx
  3e50e5:	e8 c6 99 15 00       	call   53eab0 <_Z14lua_tointegerxP9lua_StateiPi>
  3e50ea:	41 89 46 04          	mov    %eax,0x4(%r14)
  3e50ee:	31 c0                	xor    %eax,%eax
  3e50f0:	48 83 c4 08          	add    $0x8,%rsp
  3e50f4:	5b                   	pop    %rbx
  3e50f5:	41 5e                	pop    %r14
  3e50f7:	c3                   	ret
  3e50f8:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  3e50ff:	00
