
aseprite:     file format elf64-x86-64


Disassembly of section .text:

00000000003efaf0 <app::script::(anonymous namespace)::Size_get_width(lua_State*)>:
  3efaf0:	53                   	push   %rbx
  3efaf1:	48 89 fb             	mov    %rdi,%rbx
  3efaf4:	48 8d 15 df 17 b5 00 	lea    0xb517df(%rip),%rdx        # f412da <gUnaryActiveEdge+0xdb76>
  3efafb:	be 01 00 00 00       	mov    $0x1,%esi
  3efb00:	e8 7b f8 13 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3efb05:	48 63 30             	movslq (%rax),%rsi
  3efb08:	48 89 df             	mov    %rbx,%rdi
  3efb0b:	e8 00 f7 14 00       	call   53f210 <lua_pushinteger(lua_State*, long long)>
  3efb10:	b8 01 00 00 00       	mov    $0x1,%eax
  3efb15:	5b                   	pop    %rbx
  3efb16:	c3                   	ret
  3efb17:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  3efb1e:	00 00

00000000003efb20 <app::script::(anonymous namespace)::Size_set_width(lua_State*)>:
  3efb20:	41 56                	push   %r14
  3efb22:	53                   	push   %rbx
  3efb23:	50                   	push   %rax
  3efb24:	48 89 fb             	mov    %rdi,%rbx
  3efb27:	48 8d 15 ac 17 b5 00 	lea    0xb517ac(%rip),%rdx        # f412da <gUnaryActiveEdge+0xdb76>
  3efb2e:	be 01 00 00 00       	mov    $0x1,%esi
  3efb33:	e8 48 f8 13 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3efb38:	49 89 c6             	mov    %rax,%r14
  3efb3b:	48 89 df             	mov    %rbx,%rdi
  3efb3e:	be 02 00 00 00       	mov    $0x2,%esi
  3efb43:	31 d2                	xor    %edx,%edx
  3efb45:	e8 66 ef 14 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3efb4a:	41 89 06             	mov    %eax,(%r14)
  3efb4d:	31 c0                	xor    %eax,%eax
  3efb4f:	48 83 c4 08          	add    $0x8,%rsp
  3efb53:	5b                   	pop    %rbx
  3efb54:	41 5e                	pop    %r14
  3efb56:	c3                   	ret
  3efb57:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  3efb5e:	00 00

00000000003efb60 <app::script::(anonymous namespace)::Size_get_height(lua_State*)>:
  3efb60:	53                   	push   %rbx
  3efb61:	48 89 fb             	mov    %rdi,%rbx
  3efb64:	48 8d 15 6f 17 b5 00 	lea    0xb5176f(%rip),%rdx        # f412da <gUnaryActiveEdge+0xdb76>
  3efb6b:	be 01 00 00 00       	mov    $0x1,%esi
  3efb70:	e8 0b f8 13 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3efb75:	48 63 70 04          	movslq 0x4(%rax),%rsi
  3efb79:	48 89 df             	mov    %rbx,%rdi
  3efb7c:	e8 8f f6 14 00       	call   53f210 <lua_pushinteger(lua_State*, long long)>
  3efb81:	b8 01 00 00 00       	mov    $0x1,%eax
  3efb86:	5b                   	pop    %rbx
  3efb87:	c3                   	ret
  3efb88:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  3efb8f:	00

00000000003efb90 <app::script::(anonymous namespace)::Size_set_height(lua_State*)>:
  3efb90:	41 56                	push   %r14
  3efb92:	53                   	push   %rbx
  3efb93:	50                   	push   %rax
  3efb94:	48 89 fb             	mov    %rdi,%rbx
  3efb97:	48 8d 15 3c 17 b5 00 	lea    0xb5173c(%rip),%rdx        # f412da <gUnaryActiveEdge+0xdb76>
  3efb9e:	be 01 00 00 00       	mov    $0x1,%esi
  3efba3:	e8 d8 f7 13 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3efba8:	49 89 c6             	mov    %rax,%r14
  3efbab:	48 89 df             	mov    %rbx,%rdi
  3efbae:	be 02 00 00 00       	mov    $0x2,%esi
  3efbb3:	31 d2                	xor    %edx,%edx
  3efbb5:	e8 f6 ee 14 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3efbba:	41 89 46 04          	mov    %eax,0x4(%r14)
  3efbbe:	31 c0                	xor    %eax,%eax
  3efbc0:	48 83 c4 08          	add    $0x8,%rsp
  3efbc4:	5b                   	pop    %rbx
  3efbc5:	41 5e                	pop    %r14
  3efbc7:	c3                   	ret
  3efbc8:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  3efbcf:	00
