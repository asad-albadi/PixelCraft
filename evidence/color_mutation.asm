
aseprite-reference:     file format elf64-x86-64

Contents of section .data.rel.ro:
 1cbae20 98fbf300 00000000 105c3c00 00000000  .........\<.....
 1cbae30 00000000 00000000 00000000 00000000  ................
 1cbae40 3f1cf500 00000000 605c3c00 00000000  ?.......`\<.....
 1cbae50 905c3c00 00000000 077af300 00000000  .\<......z......
 1cbae60 405d3c00 00000000 705d3c00 00000000  @]<.....p]<.....
 1cbae70 1f7af300 00000000 205e3c00 00000000  .z...... ^<.....
 1cbae80 505e3c00 00000000 9871f300 00000000  P^<......q......
 1cbae90 005f3c00 00000000 305f3c00 00000000  ._<.....0_<.....
 1cbaea0 9dfbf300 00000000 705f3c00 00000000  ........p_<.....
 1cbaeb0 a05f3c00 00000000 a4fbf300 00000000  ._<.............
 1cbaec0 60603c00 00000000 90603c00 00000000  ``<......`<.....
 1cbaed0 b2fbf300 00000000 50613c00 00000000  ........Pa<.....


aseprite-reference:     file format elf64-x86-64

Contents of section .data.rel.ro:
 1cbafc0 ddfbf300 00000000 b0693c00 00000000  .........i<.....
 1cbafd0 00000000 00000000                    ........


aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

00000000003c5c60 <app::script::(anonymous namespace)::Color_get_red(lua_State*)>:
  3c5c60:	53                   	push   %rbx
  3c5c61:	48 89 fb             	mov    %rdi,%rbx
  3c5c64:	48 8d 15 e1 9e b7 00 	lea    0xb79ee1(%rip),%rdx        # f3fb4c <gUnaryActiveEdge+0xc3e8>
  3c5c6b:	be 01 00 00 00       	mov    $0x1,%esi
  3c5c70:	e8 0b 97 16 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3c5c75:	48 89 c7             	mov    %rax,%rdi
  3c5c78:	e8 c3 51 da ff       	call   16ae40 <app::Color::getRed() const>
  3c5c7d:	48 63 f0             	movslq %eax,%rsi
  3c5c80:	48 89 df             	mov    %rbx,%rdi
  3c5c83:	e8 88 95 17 00       	call   53f210 <lua_pushinteger(lua_State*, long long)>
  3c5c88:	b8 01 00 00 00       	mov    $0x1,%eax
  3c5c8d:	5b                   	pop    %rbx
  3c5c8e:	c3                   	ret
  3c5c8f:	90                   	nop

00000000003c5c90 <app::script::(anonymous namespace)::Color_set_red(lua_State*)>:
  3c5c90:	55                   	push   %rbp
  3c5c91:	41 57                	push   %r15
  3c5c93:	41 56                	push   %r14
  3c5c95:	53                   	push   %rbx
  3c5c96:	48 83 ec 38          	sub    $0x38,%rsp
  3c5c9a:	49 89 fe             	mov    %rdi,%r14
  3c5c9d:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  3c5ca4:	00 00
  3c5ca6:	48 89 44 24 30       	mov    %rax,0x30(%rsp)
  3c5cab:	48 8d 15 9a 9e b7 00 	lea    0xb79e9a(%rip),%rdx        # f3fb4c <gUnaryActiveEdge+0xc3e8>
  3c5cb2:	be 01 00 00 00       	mov    $0x1,%esi
  3c5cb7:	e8 c4 96 16 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3c5cbc:	48 89 c3             	mov    %rax,%rbx
  3c5cbf:	4c 89 f7             	mov    %r14,%rdi
  3c5cc2:	be 02 00 00 00       	mov    $0x2,%esi
  3c5cc7:	31 d2                	xor    %edx,%edx
  3c5cc9:	e8 e2 8d 17 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3c5cce:	49 89 c6             	mov    %rax,%r14
  3c5cd1:	48 89 df             	mov    %rbx,%rdi
  3c5cd4:	e8 c7 52 da ff       	call   16afa0 <app::Color::getGreen() const>
  3c5cd9:	89 c5                	mov    %eax,%ebp
  3c5cdb:	48 89 df             	mov    %rbx,%rdi
  3c5cde:	e8 3d 54 da ff       	call   16b120 <app::Color::getBlue() const>
  3c5ce3:	41 89 c7             	mov    %eax,%r15d
  3c5ce6:	48 89 df             	mov    %rbx,%rdi
  3c5ce9:	e8 b2 55 da ff       	call   16b2a0 <app::Color::getAlpha() const>
  3c5cee:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  3c5cf3:	44 89 f6             	mov    %r14d,%esi
  3c5cf6:	89 ea                	mov    %ebp,%edx
  3c5cf8:	44 89 f9             	mov    %r15d,%ecx
  3c5cfb:	41 89 c0             	mov    %eax,%r8d
  3c5cfe:	e8 bd 48 da ff       	call   16a5c0 <app::Color::fromRgb(int, int, int, int)>
  3c5d03:	48 8b 44 24 28       	mov    0x28(%rsp),%rax
  3c5d08:	48 89 43 20          	mov    %rax,0x20(%rbx)
  3c5d0c:	0f 10 44 24 08       	movups 0x8(%rsp),%xmm0
  3c5d11:	0f 10 4c 24 18       	movups 0x18(%rsp),%xmm1
  3c5d16:	0f 11 4b 10          	movups %xmm1,0x10(%rbx)
  3c5d1a:	0f 11 03             	movups %xmm0,(%rbx)
  3c5d1d:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  3c5d24:	00 00
  3c5d26:	48 3b 44 24 30       	cmp    0x30(%rsp),%rax
  3c5d2b:	75 0d                	jne    3c5d3a <app::script::(anonymous namespace)::Color_set_red(lua_State*)+0xaa>
  3c5d2d:	31 c0                	xor    %eax,%eax
  3c5d2f:	48 83 c4 38          	add    $0x38,%rsp
  3c5d33:	5b                   	pop    %rbx
  3c5d34:	41 5e                	pop    %r14
  3c5d36:	41 5f                	pop    %r15
  3c5d38:	5d                   	pop    %rbp
  3c5d39:	c3                   	ret
  3c5d3a:	e8 a1 08 d8 ff       	call   1465e0 <__stack_chk_fail@plt>
  3c5d3f:	90                   	nop

00000000003c5d40 <app::script::(anonymous namespace)::Color_get_green(lua_State*)>:
  3c5d40:	53                   	push   %rbx
  3c5d41:	48 89 fb             	mov    %rdi,%rbx
  3c5d44:	48 8d 15 01 9e b7 00 	lea    0xb79e01(%rip),%rdx        # f3fb4c <gUnaryActiveEdge+0xc3e8>
  3c5d4b:	be 01 00 00 00       	mov    $0x1,%esi
  3c5d50:	e8 2b 96 16 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3c5d55:	48 89 c7             	mov    %rax,%rdi
  3c5d58:	e8 43 52 da ff       	call   16afa0 <app::Color::getGreen() const>
  3c5d5d:	48 63 f0             	movslq %eax,%rsi
  3c5d60:	48 89 df             	mov    %rbx,%rdi
  3c5d63:	e8 a8 94 17 00       	call   53f210 <lua_pushinteger(lua_State*, long long)>
  3c5d68:	b8 01 00 00 00       	mov    $0x1,%eax
  3c5d6d:	5b                   	pop    %rbx
  3c5d6e:	c3                   	ret
  3c5d6f:	90                   	nop

00000000003c5d70 <app::script::(anonymous namespace)::Color_set_green(lua_State*)>:
  3c5d70:	55                   	push   %rbp
  3c5d71:	41 57                	push   %r15
  3c5d73:	41 56                	push   %r14
  3c5d75:	53                   	push   %rbx
  3c5d76:	48 83 ec 38          	sub    $0x38,%rsp
  3c5d7a:	49 89 fe             	mov    %rdi,%r14
  3c5d7d:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  3c5d84:	00 00
  3c5d86:	48 89 44 24 30       	mov    %rax,0x30(%rsp)
  3c5d8b:	48 8d 15 ba 9d b7 00 	lea    0xb79dba(%rip),%rdx        # f3fb4c <gUnaryActiveEdge+0xc3e8>
  3c5d92:	be 01 00 00 00       	mov    $0x1,%esi
  3c5d97:	e8 e4 95 16 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3c5d9c:	48 89 c3             	mov    %rax,%rbx
  3c5d9f:	48 89 c7             	mov    %rax,%rdi
  3c5da2:	e8 99 50 da ff       	call   16ae40 <app::Color::getRed() const>
  3c5da7:	89 c5                	mov    %eax,%ebp
  3c5da9:	4c 89 f7             	mov    %r14,%rdi
  3c5dac:	be 02 00 00 00       	mov    $0x2,%esi
  3c5db1:	31 d2                	xor    %edx,%edx
  3c5db3:	e8 f8 8c 17 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3c5db8:	49 89 c6             	mov    %rax,%r14
  3c5dbb:	48 89 df             	mov    %rbx,%rdi
  3c5dbe:	e8 5d 53 da ff       	call   16b120 <app::Color::getBlue() const>
  3c5dc3:	41 89 c7             	mov    %eax,%r15d
  3c5dc6:	48 89 df             	mov    %rbx,%rdi
  3c5dc9:	e8 d2 54 da ff       	call   16b2a0 <app::Color::getAlpha() const>
  3c5dce:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  3c5dd3:	89 ee                	mov    %ebp,%esi
  3c5dd5:	44 89 f2             	mov    %r14d,%edx
  3c5dd8:	44 89 f9             	mov    %r15d,%ecx
  3c5ddb:	41 89 c0             	mov    %eax,%r8d
  3c5dde:	e8 dd 47 da ff       	call   16a5c0 <app::Color::fromRgb(int, int, int, int)>
  3c5de3:	48 8b 44 24 28       	mov    0x28(%rsp),%rax
  3c5de8:	48 89 43 20          	mov    %rax,0x20(%rbx)
  3c5dec:	0f 10 44 24 08       	movups 0x8(%rsp),%xmm0
  3c5df1:	0f 10 4c 24 18       	movups 0x18(%rsp),%xmm1
  3c5df6:	0f 11 4b 10          	movups %xmm1,0x10(%rbx)
  3c5dfa:	0f 11 03             	movups %xmm0,(%rbx)
  3c5dfd:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  3c5e04:	00 00
  3c5e06:	48 3b 44 24 30       	cmp    0x30(%rsp),%rax
  3c5e0b:	75 0d                	jne    3c5e1a <app::script::(anonymous namespace)::Color_set_green(lua_State*)+0xaa>
  3c5e0d:	31 c0                	xor    %eax,%eax
  3c5e0f:	48 83 c4 38          	add    $0x38,%rsp
  3c5e13:	5b                   	pop    %rbx
  3c5e14:	41 5e                	pop    %r14
  3c5e16:	41 5f                	pop    %r15
  3c5e18:	5d                   	pop    %rbp
  3c5e19:	c3                   	ret
  3c5e1a:	e8 c1 07 d8 ff       	call   1465e0 <__stack_chk_fail@plt>
  3c5e1f:	90                   	nop

00000000003c5e20 <app::script::(anonymous namespace)::Color_get_blue(lua_State*)>:
  3c5e20:	53                   	push   %rbx
  3c5e21:	48 89 fb             	mov    %rdi,%rbx
  3c5e24:	48 8d 15 21 9d b7 00 	lea    0xb79d21(%rip),%rdx        # f3fb4c <gUnaryActiveEdge+0xc3e8>
  3c5e2b:	be 01 00 00 00       	mov    $0x1,%esi
  3c5e30:	e8 4b 95 16 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3c5e35:	48 89 c7             	mov    %rax,%rdi
  3c5e38:	e8 e3 52 da ff       	call   16b120 <app::Color::getBlue() const>
  3c5e3d:	48 63 f0             	movslq %eax,%rsi
  3c5e40:	48 89 df             	mov    %rbx,%rdi
  3c5e43:	e8 c8 93 17 00       	call   53f210 <lua_pushinteger(lua_State*, long long)>
  3c5e48:	b8 01 00 00 00       	mov    $0x1,%eax
  3c5e4d:	5b                   	pop    %rbx
  3c5e4e:	c3                   	ret
  3c5e4f:	90                   	nop

00000000003c5e50 <app::script::(anonymous namespace)::Color_set_blue(lua_State*)>:
  3c5e50:	55                   	push   %rbp
  3c5e51:	41 57                	push   %r15
  3c5e53:	41 56                	push   %r14
  3c5e55:	53                   	push   %rbx
  3c5e56:	48 83 ec 38          	sub    $0x38,%rsp
  3c5e5a:	49 89 fe             	mov    %rdi,%r14
  3c5e5d:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  3c5e64:	00 00
  3c5e66:	48 89 44 24 30       	mov    %rax,0x30(%rsp)
  3c5e6b:	48 8d 15 da 9c b7 00 	lea    0xb79cda(%rip),%rdx        # f3fb4c <gUnaryActiveEdge+0xc3e8>
  3c5e72:	be 01 00 00 00       	mov    $0x1,%esi
  3c5e77:	e8 04 95 16 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3c5e7c:	48 89 c3             	mov    %rax,%rbx
  3c5e7f:	48 89 c7             	mov    %rax,%rdi
  3c5e82:	e8 b9 4f da ff       	call   16ae40 <app::Color::getRed() const>
  3c5e87:	89 c5                	mov    %eax,%ebp
  3c5e89:	48 89 df             	mov    %rbx,%rdi
  3c5e8c:	e8 0f 51 da ff       	call   16afa0 <app::Color::getGreen() const>
  3c5e91:	41 89 c7             	mov    %eax,%r15d
  3c5e94:	4c 89 f7             	mov    %r14,%rdi
  3c5e97:	be 02 00 00 00       	mov    $0x2,%esi
  3c5e9c:	31 d2                	xor    %edx,%edx
  3c5e9e:	e8 0d 8c 17 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3c5ea3:	49 89 c6             	mov    %rax,%r14
  3c5ea6:	48 89 df             	mov    %rbx,%rdi
  3c5ea9:	e8 f2 53 da ff       	call   16b2a0 <app::Color::getAlpha() const>
  3c5eae:	48 8d 7c 24 08       	lea    0x8(%rsp),%rdi
  3c5eb3:	89 ee                	mov    %ebp,%esi
  3c5eb5:	44 89 fa             	mov    %r15d,%edx
  3c5eb8:	44 89 f1             	mov    %r14d,%ecx
  3c5ebb:	41 89 c0             	mov    %eax,%r8d
  3c5ebe:	e8 fd 46 da ff       	call   16a5c0 <app::Color::fromRgb(int, int, int, int)>
  3c5ec3:	48 8b 44 24 28       	mov    0x28(%rsp),%rax
  3c5ec8:	48 89 43 20          	mov    %rax,0x20(%rbx)
  3c5ecc:	0f 10 44 24 08       	movups 0x8(%rsp),%xmm0
  3c5ed1:	0f 10 4c 24 18       	movups 0x18(%rsp),%xmm1
  3c5ed6:	0f 11 4b 10          	movups %xmm1,0x10(%rbx)
  3c5eda:	0f 11 03             	movups %xmm0,(%rbx)
  3c5edd:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  3c5ee4:	00 00
  3c5ee6:	48 3b 44 24 30       	cmp    0x30(%rsp),%rax
  3c5eeb:	75 0d                	jne    3c5efa <app::script::(anonymous namespace)::Color_set_blue(lua_State*)+0xaa>
  3c5eed:	31 c0                	xor    %eax,%eax
  3c5eef:	48 83 c4 38          	add    $0x38,%rsp
  3c5ef3:	5b                   	pop    %rbx
  3c5ef4:	41 5e                	pop    %r14
  3c5ef6:	41 5f                	pop    %r15
  3c5ef8:	5d                   	pop    %rbp
  3c5ef9:	c3                   	ret
  3c5efa:	e8 e1 06 d8 ff       	call   1465e0 <__stack_chk_fail@plt>
  3c5eff:	90                   	nop

00000000003c5f00 <app::script::(anonymous namespace)::Color_get_alpha(lua_State*)>:
  3c5f00:	53                   	push   %rbx
  3c5f01:	48 89 fb             	mov    %rdi,%rbx
  3c5f04:	48 8d 15 41 9c b7 00 	lea    0xb79c41(%rip),%rdx        # f3fb4c <gUnaryActiveEdge+0xc3e8>
  3c5f0b:	be 01 00 00 00       	mov    $0x1,%esi
  3c5f10:	e8 6b 94 16 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3c5f15:	48 89 c7             	mov    %rax,%rdi
  3c5f18:	e8 83 53 da ff       	call   16b2a0 <app::Color::getAlpha() const>
  3c5f1d:	48 63 f0             	movslq %eax,%rsi
  3c5f20:	48 89 df             	mov    %rbx,%rdi
  3c5f23:	e8 e8 92 17 00       	call   53f210 <lua_pushinteger(lua_State*, long long)>
  3c5f28:	b8 01 00 00 00       	mov    $0x1,%eax
  3c5f2d:	5b                   	pop    %rbx
  3c5f2e:	c3                   	ret
  3c5f2f:	90                   	nop

00000000003c5f30 <app::script::(anonymous namespace)::Color_set_alpha(lua_State*)>:
  3c5f30:	41 56                	push   %r14
  3c5f32:	53                   	push   %rbx
  3c5f33:	50                   	push   %rax
  3c5f34:	48 89 fb             	mov    %rdi,%rbx
  3c5f37:	48 8d 15 0e 9c b7 00 	lea    0xb79c0e(%rip),%rdx        # f3fb4c <gUnaryActiveEdge+0xc3e8>
  3c5f3e:	be 01 00 00 00       	mov    $0x1,%esi
  3c5f43:	e8 38 94 16 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3c5f48:	49 89 c6             	mov    %rax,%r14
  3c5f4b:	48 89 df             	mov    %rbx,%rdi
  3c5f4e:	be 02 00 00 00       	mov    $0x2,%esi
  3c5f53:	31 d2                	xor    %edx,%edx
  3c5f55:	e8 56 8b 17 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3c5f5a:	4c 89 f7             	mov    %r14,%rdi
  3c5f5d:	89 c6                	mov    %eax,%esi
  3c5f5f:	e8 5c 6e da ff       	call   16cdc0 <app::Color::setAlpha(int)>
  3c5f64:	31 c0                	xor    %eax,%eax
  3c5f66:	48 83 c4 08          	add    $0x8,%rsp
  3c5f6a:	5b                   	pop    %rbx
  3c5f6b:	41 5e                	pop    %r14
  3c5f6d:	c3                   	ret
  3c5f6e:	66 90                	xchg   %ax,%ax


aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

00000000003c69b0 <app::script::(anonymous namespace)::Color_get_rgbaPixel(lua_State*)>:
  3c69b0:	53                   	push   %rbx
  3c69b1:	48 83 ec 20          	sub    $0x20,%rsp
  3c69b5:	48 89 fb             	mov    %rdi,%rbx
  3c69b8:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  3c69bf:	00 00
  3c69c1:	48 89 44 24 18       	mov    %rax,0x18(%rsp)
  3c69c6:	48 8d 15 7f 91 b7 00 	lea    0xb7917f(%rip),%rdx        # f3fb4c <gUnaryActiveEdge+0xc3e8>
  3c69cd:	be 01 00 00 00       	mov    $0x1,%esi
  3c69d2:	e8 a9 89 16 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3c69d7:	48 c7 44 24 0c 01 00 	movq   $0x1,0xc(%rsp)
  3c69de:	00 00
  3c69e0:	c7 44 24 14 00 00 00 	movl   $0x0,0x14(%rsp)
  3c69e7:	00
  3c69e8:	48 8d 74 24 0c       	lea    0xc(%rsp),%rsi
  3c69ed:	48 89 c7             	mov    %rax,%rdi
  3c69f0:	e8 9b 72 da ff       	call   16dc90 <app::color_utils::color_for_target_mask(app::Color const&, app::ColorTarget const&)>
  3c69f5:	89 c6                	mov    %eax,%esi
  3c69f7:	48 89 df             	mov    %rbx,%rdi
  3c69fa:	e8 11 88 17 00       	call   53f210 <lua_pushinteger(lua_State*, long long)>
  3c69ff:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  3c6a06:	00 00
  3c6a08:	48 3b 44 24 18       	cmp    0x18(%rsp),%rax
  3c6a0d:	75 0b                	jne    3c6a1a <app::script::(anonymous namespace)::Color_get_rgbaPixel(lua_State*)+0x6a>
  3c6a0f:	b8 01 00 00 00       	mov    $0x1,%eax
  3c6a14:	48 83 c4 20          	add    $0x20,%rsp
  3c6a18:	5b                   	pop    %rbx
  3c6a19:	c3                   	ret
  3c6a1a:	e8 c1 fb d7 ff       	call   1465e0 <__stack_chk_fail@plt>
  3c6a1f:	90                   	nop


aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

000000000016dc90 <app::color_utils::color_for_target_mask(app::Color const&, app::ColorTarget const&)>:
  16dc90:	55                   	push   %rbp
  16dc91:	41 57                	push   %r15
  16dc93:	41 56                	push   %r14
  16dc95:	41 54                	push   %r12
  16dc97:	53                   	push   %rbx
  16dc98:	8b 0f                	mov    (%rdi),%ecx
  16dc9a:	85 c9                	test   %ecx,%ecx
  16dc9c:	74 66                	je     16dd04 <app::color_utils::color_for_target_mask(app::Color const&, app::ColorTarget const&)+0x74>
  16dc9e:	8b 56 04             	mov    0x4(%rsi),%edx
  16dca1:	b8 ff ff ff ff       	mov    $0xffffffff,%eax
  16dca6:	48 83 fa 04          	cmp    $0x4,%rdx
  16dcaa:	0f 87 89 00 00 00    	ja     16dd39 <app::color_utils::color_for_target_mask(app::Color const&, app::ColorTarget const&)+0xa9>
  16dcb0:	4c 8d 05 ed 3e e1 00 	lea    0xe13eed(%rip),%r8        # f81ba4 <typeinfo name for app::initialize_color_spaces(app::Preferences&)::$_0+0x60>
  16dcb7:	49 63 14 90          	movslq (%r8,%rdx,4),%rdx
  16dcbb:	4c 01 c2             	add    %r8,%rdx
  16dcbe:	ff e2                	jmp    *%rdx
  16dcc0:	48 89 fb             	mov    %rdi,%rbx
  16dcc3:	e8 78 d1 ff ff       	call   16ae40 <app::Color::getRed() const>
  16dcc8:	89 c5                	mov    %eax,%ebp
  16dcca:	48 89 df             	mov    %rbx,%rdi
  16dccd:	e8 ce d2 ff ff       	call   16afa0 <app::Color::getGreen() const>
  16dcd2:	41 89 c6             	mov    %eax,%r14d
  16dcd5:	48 89 df             	mov    %rbx,%rdi
  16dcd8:	e8 43 d4 ff ff       	call   16b120 <app::Color::getBlue() const>
  16dcdd:	41 89 c7             	mov    %eax,%r15d
  16dce0:	48 89 df             	mov    %rbx,%rdi
  16dce3:	e8 b8 d5 ff ff       	call   16b2a0 <app::Color::getAlpha() const>
  16dce8:	40 0f b6 cd          	movzbl %bpl,%ecx
  16dcec:	41 c1 e6 08          	shl    $0x8,%r14d
  16dcf0:	41 0f b7 d6          	movzwl %r14w,%edx
  16dcf4:	09 ca                	or     %ecx,%edx
  16dcf6:	41 0f b6 cf          	movzbl %r15b,%ecx
  16dcfa:	c1 e1 10             	shl    $0x10,%ecx
  16dcfd:	09 d1                	or     %edx,%ecx
  16dcff:	c1 e0 18             	shl    $0x18,%eax
  16dd02:	eb 33                	jmp    16dd37 <app::color_utils::color_for_target_mask(app::Color const&, app::ColorTarget const&)+0xa7>
  16dd04:	8b 46 08             	mov    0x8(%rsi),%eax
  16dd07:	eb 30                	jmp    16dd39 <app::color_utils::color_for_target_mask(app::Color const&, app::ColorTarget const&)+0xa9>
  16dd09:	83 f9 05             	cmp    $0x5,%ecx
  16dd0c:	75 41                	jne    16dd4f <app::color_utils::color_for_target_mask(app::Color const&, app::ColorTarget const&)+0xbf>
  16dd0e:	5b                   	pop    %rbx
  16dd0f:	41 5c                	pop    %r12
  16dd11:	41 5e                	pop    %r14
  16dd13:	41 5f                	pop    %r15
  16dd15:	5d                   	pop    %rbp
  16dd16:	e9 95 ee ff ff       	jmp    16cbb0 <app::Color::getIndex() const>
  16dd1b:	48 89 fb             	mov    %rdi,%rbx
  16dd1e:	e8 8d e5 ff ff       	call   16c2b0 <app::Color::getGray() const>
  16dd23:	89 c5                	mov    %eax,%ebp
  16dd25:	48 89 df             	mov    %rbx,%rdi
  16dd28:	e8 73 d5 ff ff       	call   16b2a0 <app::Color::getAlpha() const>
  16dd2d:	40 0f b6 cd          	movzbl %bpl,%ecx
  16dd31:	c1 e0 08             	shl    $0x8,%eax
  16dd34:	0f b7 c0             	movzwl %ax,%eax
  16dd37:	09 c8                	or     %ecx,%eax
  16dd39:	5b                   	pop    %rbx
  16dd3a:	41 5c                	pop    %r12
  16dd3c:	41 5e                	pop    %r14
  16dd3e:	41 5f                	pop    %r15
  16dd40:	5d                   	pop    %rbp
  16dd41:	c3                   	ret
  16dd42:	5b                   	pop    %rbx
  16dd43:	41 5c                	pop    %r12
  16dd45:	41 5e                	pop    %r14
  16dd47:	41 5f                	pop    %r15
  16dd49:	5d                   	pop    %rbp
  16dd4a:	e9 61 f0 ff ff       	jmp    16cdb0 <app::Color::getTile() const>
  16dd4f:	49 89 ff             	mov    %rdi,%r15
  16dd52:	49 89 f4             	mov    %rsi,%r12
  16dd55:	e8 e6 d0 ff ff       	call   16ae40 <app::Color::getRed() const>
  16dd5a:	89 c3                	mov    %eax,%ebx
  16dd5c:	4c 89 ff             	mov    %r15,%rdi
  16dd5f:	e8 3c d2 ff ff       	call   16afa0 <app::Color::getGreen() const>
  16dd64:	89 c5                	mov    %eax,%ebp
  16dd66:	4c 89 ff             	mov    %r15,%rdi
  16dd69:	e8 b2 d3 ff ff       	call   16b120 <app::Color::getBlue() const>
  16dd6e:	41 89 c6             	mov    %eax,%r14d
  16dd71:	4c 89 ff             	mov    %r15,%rdi
  16dd74:	e8 27 d5 ff ff       	call   16b2a0 <app::Color::getAlpha() const>
  16dd79:	4c 89 e1             	mov    %r12,%rcx
  16dd7c:	41 89 c7             	mov    %eax,%r15d
  16dd7f:	41 bc ff ff ff ff    	mov    $0xffffffff,%r12d
  16dd85:	83 39 01             	cmpl   $0x1,(%rcx)
  16dd88:	75 04                	jne    16dd8e <app::color_utils::color_for_target_mask(app::Color const&, app::ColorTarget const&)+0xfe>
  16dd8a:	44 8b 61 08          	mov    0x8(%rcx),%r12d
  16dd8e:	e8 b5 47 07 00       	call   1e2548 <app::get_current_palette()>
  16dd93:	48 89 c7             	mov    %rax,%rdi
  16dd96:	89 de                	mov    %ebx,%esi
  16dd98:	89 ea                	mov    %ebp,%edx
  16dd9a:	44 89 f1             	mov    %r14d,%ecx
  16dd9d:	45 89 f8             	mov    %r15d,%r8d
  16dda0:	45 89 e1             	mov    %r12d,%r9d
  16dda3:	e8 88 d6 49 00       	call   60b430 <doc::Palette::findExactMatch(int, int, int, int, int) const>
  16dda8:	85 c0                	test   %eax,%eax
  16ddaa:	79 8d                	jns    16dd39 <app::color_utils::color_for_target_mask(app::Color const&, app::ColorTarget const&)+0xa9>
  16ddac:	e8 97 47 07 00       	call   1e2548 <app::get_current_palette()>
  16ddb1:	48 89 c7             	mov    %rax,%rdi
  16ddb4:	89 de                	mov    %ebx,%esi
  16ddb6:	89 ea                	mov    %ebp,%edx
  16ddb8:	44 89 f1             	mov    %r14d,%ecx
  16ddbb:	45 89 f8             	mov    %r15d,%r8d
  16ddbe:	45 89 e1             	mov    %r12d,%r9d
  16ddc1:	5b                   	pop    %rbx
  16ddc2:	41 5c                	pop    %r12
  16ddc4:	41 5e                	pop    %r14
  16ddc6:	41 5f                	pop    %r15
  16ddc8:	5d                   	pop    %rbp
  16ddc9:	e9 b2 d9 49 00       	jmp    60b780 <doc::Palette::findBestfit(int, int, int, int, int) const>
  16ddce:	66 90                	xchg   %ax,%ax


aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

000000000016a5c0 <app::Color::fromRgb(int, int, int, int)>:
  16a5c0:	48 89 f8             	mov    %rdi,%rax
  16a5c3:	c7 07 01 00 00 00    	movl   $0x1,(%rdi)
  16a5c9:	89 77 08             	mov    %esi,0x8(%rdi)
  16a5cc:	89 57 0c             	mov    %edx,0xc(%rdi)
  16a5cf:	89 4f 10             	mov    %ecx,0x10(%rdi)
  16a5d2:	44 89 47 14          	mov    %r8d,0x14(%rdi)
  16a5d6:	c3                   	ret
  16a5d7:	90                   	nop
  16a5d8:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  16a5df:	00

000000000016a5e0 <app::Color::fromHsv(double, double, double, int)>:
  16a5e0:	48 89 f8             	mov    %rdi,%rax
  16a5e3:	c7 07 02 00 00 00    	movl   $0x2,(%rdi)
  16a5e9:	f2 0f 11 47 08       	movsd  %xmm0,0x8(%rdi)
  16a5ee:	f2 0f 11 4f 10       	movsd  %xmm1,0x10(%rdi)
  16a5f3:	f2 0f 11 57 18       	movsd  %xmm2,0x18(%rdi)
  16a5f8:	89 77 20             	mov    %esi,0x20(%rdi)
  16a5fb:	c3                   	ret
  16a5fc:	0f 1f 40 00          	nopl   0x0(%rax)

aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

000000000016cdc0 <app::Color::setAlpha(int)>:
  16cdc0:	55                   	push   %rbp
  16cdc1:	41 57                	push   %r15
  16cdc3:	41 56                	push   %r14
  16cdc5:	53                   	push   %rbx
  16cdc6:	50                   	push   %rax
  16cdc7:	31 c0                	xor    %eax,%eax
  16cdc9:	85 f6                	test   %esi,%esi
  16cdcb:	0f 4f c6             	cmovg  %esi,%eax
  16cdce:	3d ff 00 00 00       	cmp    $0xff,%eax
  16cdd3:	41 bf ff 00 00 00    	mov    $0xff,%r15d
  16cdd9:	44 0f 4c f8          	cmovl  %eax,%r15d
  16cddd:	8b 07                	mov    (%rdi),%eax
  16cddf:	ff c8                	dec    %eax
  16cde1:	83 f8 04             	cmp    $0x4,%eax
  16cde4:	77 53                	ja     16ce39 <app::Color::setAlpha(int)+0x79>
  16cde6:	48 8d 0d 2f 4d e1 00 	lea    0xe14d2f(%rip),%rcx        # f81b1c <typeinfo name for app::PreviewCliDelegate+0x23c>
  16cded:	48 63 04 81          	movslq (%rcx,%rax,4),%rax
  16cdf1:	48 01 c8             	add    %rcx,%rax
  16cdf4:	ff e0                	jmp    *%rax
  16cdf6:	44 89 7f 20          	mov    %r15d,0x20(%rdi)
  16cdfa:	eb 3d                	jmp    16ce39 <app::Color::setAlpha(int)+0x79>
  16cdfc:	48 89 fb             	mov    %rdi,%rbx
  16cdff:	e8 3c e0 ff ff       	call   16ae40 <app::Color::getRed() const>
  16ce04:	89 c5                	mov    %eax,%ebp
  16ce06:	48 89 df             	mov    %rbx,%rdi
  16ce09:	e8 92 e1 ff ff       	call   16afa0 <app::Color::getGreen() const>
  16ce0e:	41 89 c6             	mov    %eax,%r14d
  16ce11:	48 89 df             	mov    %rbx,%rdi
  16ce14:	e8 07 e3 ff ff       	call   16b120 <app::Color::getBlue() const>
  16ce19:	c7 03 01 00 00 00    	movl   $0x1,(%rbx)
  16ce1f:	89 6b 08             	mov    %ebp,0x8(%rbx)
  16ce22:	44 89 73 0c          	mov    %r14d,0xc(%rbx)
  16ce26:	89 43 10             	mov    %eax,0x10(%rbx)
  16ce29:	44 89 7b 14          	mov    %r15d,0x14(%rbx)
  16ce2d:	eb 0a                	jmp    16ce39 <app::Color::setAlpha(int)+0x79>
  16ce2f:	44 89 7f 14          	mov    %r15d,0x14(%rdi)
  16ce33:	eb 04                	jmp    16ce39 <app::Color::setAlpha(int)+0x79>
  16ce35:	44 89 7f 0c          	mov    %r15d,0xc(%rdi)
  16ce39:	48 83 c4 08          	add    $0x8,%rsp
  16ce3d:	5b                   	pop    %rbx
  16ce3e:	41 5e                	pop    %r14
  16ce40:	41 5f                	pop    %r15
  16ce42:	5d                   	pop    %rbp
  16ce43:	c3                   	ret
  16ce44:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  16ce4b:	00 00 00
  16ce4e:	66 90                	xchg   %ax,%ax

aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

000000000016ae40 <app::Color::getRed() const>:
  16ae40:	53                   	push   %rbx
  16ae41:	48 83 ec 30          	sub    $0x30,%rsp
  16ae45:	64 48 8b 04 25 28 00 	mov    %fs:0x28,%rax
  16ae4c:	00 00
  16ae4e:	48 89 44 24 28       	mov    %rax,0x28(%rsp)
  16ae53:	8b 07                	mov    (%rdi),%eax
  16ae55:	48 83 f8 05          	cmp    $0x5,%rax
  16ae59:	0f 87 03 01 00 00    	ja     16af62 <app::Color::getRed() const+0x122>
  16ae5f:	48 8d 0d f6 6a e1 00 	lea    0xe16af6(%rip),%rcx        # f8195c <typeinfo name for app::PreviewCliDelegate+0x7c>
  16ae66:	48 63 14 81          	movslq (%rcx,%rax,4),%rdx
  16ae6a:	48 01 ca             	add    %rcx,%rdx
  16ae6d:	ff e2                	jmp    *%rdx
  16ae6f:	8b 47 08             	mov    0x8(%rdi),%eax
  16ae72:	64 48 8b 0c 25 28 00 	mov    %fs:0x28,%rcx
  16ae79:	00 00
  16ae7b:	48 3b 4c 24 28       	cmp    0x28(%rsp),%rcx
  16ae80:	0f 84 f5 00 00 00    	je     16af7b <app::Color::getRed() const+0x13b>
  16ae86:	e8 55 b7 fd ff       	call   1465e0 <__stack_chk_fail@plt>
  16ae8b:	f2 0f 10 47 08       	movsd  0x8(%rdi),%xmm0
  16ae90:	f2 0f 10 4f 10       	movsd  0x10(%rdi),%xmm1
  16ae95:	f2 0f 10 57 18       	movsd  0x18(%rdi),%xmm2
  16ae9a:	48 89 e3             	mov    %rsp,%rbx
  16ae9d:	48 89 df             	mov    %rbx,%rdi
  16aea0:	e8 0b 67 52 00       	call   6915b0 <gfx::Hsv::Hsv(double, double, double)>
  16aea5:	48 8d 7c 24 1c       	lea    0x1c(%rsp),%rdi
  16aeaa:	48 89 de             	mov    %rbx,%rsi
  16aead:	e8 fe 68 52 00       	call   6917b0 <gfx::Rgb::Rgb(gfx::Hsv const&)>
  16aeb2:	8b 44 24 1c          	mov    0x1c(%rsp),%eax
  16aeb6:	64 48 8b 0c 25 28 00 	mov    %fs:0x28,%rcx
  16aebd:	00 00
  16aebf:	48 3b 4c 24 28       	cmp    0x28(%rsp),%rcx
  16aec4:	0f 84 b1 00 00 00    	je     16af7b <app::Color::getRed() const+0x13b>
  16aeca:	eb ba                	jmp    16ae86 <app::Color::getRed() const+0x46>
  16aecc:	f2 0f 10 47 08       	movsd  0x8(%rdi),%xmm0
  16aed1:	f2 0f 10 4f 10       	movsd  0x10(%rdi),%xmm1
  16aed6:	f2 0f 10 57 18       	movsd  0x18(%rdi),%xmm2
  16aedb:	48 89 e3             	mov    %rsp,%rbx
  16aede:	48 89 df             	mov    %rbx,%rdi
  16aee1:	e8 8a 64 52 00       	call   691370 <gfx::Hsl::Hsl(double, double, double)>
  16aee6:	48 8d 7c 24 1c       	lea    0x1c(%rsp),%rdi
  16aeeb:	48 89 de             	mov    %rbx,%rsi
  16aeee:	e8 ed 69 52 00       	call   6918e0 <gfx::Rgb::Rgb(gfx::Hsl const&)>
  16aef3:	8b 44 24 1c          	mov    0x1c(%rsp),%eax
  16aef7:	64 48 8b 0c 25 28 00 	mov    %fs:0x28,%rcx
  16aefe:	00 00
  16af00:	48 3b 4c 24 28       	cmp    0x28(%rsp),%rcx
  16af05:	74 74                	je     16af7b <app::Color::getRed() const+0x13b>
  16af07:	e9 7a ff ff ff       	jmp    16ae86 <app::Color::getRed() const+0x46>
  16af0c:	48 63 5f 08          	movslq 0x8(%rdi),%rbx
  16af10:	31 c0                	xor    %eax,%eax
  16af12:	48 85 db             	test   %rbx,%rbx
  16af15:	78 50                	js     16af67 <app::Color::getRed() const+0x127>
  16af17:	e8 2c 76 07 00       	call   1e2548 <app::get_current_palette()>
  16af1c:	48 8b 48 28          	mov    0x28(%rax),%rcx
  16af20:	48 2b 48 20          	sub    0x20(%rax),%rcx
  16af24:	48 c1 e9 02          	shr    $0x2,%rcx
  16af28:	39 cb                	cmp    %ecx,%ebx
  16af2a:	7d 55                	jge    16af81 <app::Color::getRed() const+0x141>
  16af2c:	e8 17 76 07 00       	call   1e2548 <app::get_current_palette()>
  16af31:	48 8b 48 20          	mov    0x20(%rax),%rcx
  16af35:	48 8b 40 28          	mov    0x28(%rax),%rax
  16af39:	48 29 c8             	sub    %rcx,%rax
  16af3c:	48 c1 e8 02          	shr    $0x2,%rax
  16af40:	39 c3                	cmp    %eax,%ebx
  16af42:	b8 00 00 00 00       	mov    $0x0,%eax
  16af47:	7d 1e                	jge    16af67 <app::Color::getRed() const+0x127>
  16af49:	0f b6 04 99          	movzbl (%rcx,%rbx,4),%eax
  16af4d:	64 48 8b 0c 25 28 00 	mov    %fs:0x28,%rcx
  16af54:	00 00
  16af56:	48 3b 4c 24 28       	cmp    0x28(%rsp),%rcx
  16af5b:	74 1e                	je     16af7b <app::Color::getRed() const+0x13b>
  16af5d:	e9 24 ff ff ff       	jmp    16ae86 <app::Color::getRed() const+0x46>
  16af62:	b8 ff ff ff ff       	mov    $0xffffffff,%eax
  16af67:	64 48 8b 0c 25 28 00 	mov    %fs:0x28,%rcx
  16af6e:	00 00
  16af70:	48 3b 4c 24 28       	cmp    0x28(%rsp),%rcx
  16af75:	0f 85 0b ff ff ff    	jne    16ae86 <app::Color::getRed() const+0x46>
  16af7b:	48 83 c4 30          	add    $0x30,%rsp
  16af7f:	5b                   	pop    %rbx
