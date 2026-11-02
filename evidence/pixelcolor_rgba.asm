
aseprite-reference:     file format elf64-x86-64


Disassembly of section .text:

00000000003e2100 <app::script::(anonymous namespace)::PixelColor_rgba(lua_State*)>:
  3e2100:	41 57                	push   %r15
  3e2102:	41 56                	push   %r14
  3e2104:	41 54                	push   %r12
  3e2106:	53                   	push   %rbx
  3e2107:	50                   	push   %rax
  3e2108:	48 89 fb             	mov    %rdi,%rbx
  3e210b:	be 01 00 00 00       	mov    $0x1,%esi
  3e2110:	31 d2                	xor    %edx,%edx
  3e2112:	e8 99 c9 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3e2117:	49 89 c6             	mov    %rax,%r14
  3e211a:	48 89 df             	mov    %rbx,%rdi
  3e211d:	be 02 00 00 00       	mov    $0x2,%esi
  3e2122:	31 d2                	xor    %edx,%edx
  3e2124:	e8 87 c9 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3e2129:	49 89 c7             	mov    %rax,%r15
  3e212c:	48 89 df             	mov    %rbx,%rdi
  3e212f:	be 03 00 00 00       	mov    $0x3,%esi
  3e2134:	31 d2                	xor    %edx,%edx
  3e2136:	e8 75 c9 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3e213b:	49 89 c4             	mov    %rax,%r12
  3e213e:	48 89 df             	mov    %rbx,%rdi
  3e2141:	be 04 00 00 00       	mov    $0x4,%esi
  3e2146:	e8 25 c1 15 00       	call   53e270 <lua_type(lua_State*, int)>
  3e214b:	85 c0                	test   %eax,%eax
  3e214d:	7e 14                	jle    3e2163 <app::script::(anonymous namespace)::PixelColor_rgba(lua_State*)+0x63>
  3e214f:	48 89 df             	mov    %rbx,%rdi
  3e2152:	be 04 00 00 00       	mov    $0x4,%esi
  3e2157:	31 d2                	xor    %edx,%edx
  3e2159:	e8 52 c9 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3e215e:	c1 e0 18             	shl    $0x18,%eax
  3e2161:	eb 05                	jmp    3e2168 <app::script::(anonymous namespace)::PixelColor_rgba(lua_State*)+0x68>
  3e2163:	b8 00 00 00 ff       	mov    $0xff000000,%eax
  3e2168:	41 0f b6 ce          	movzbl %r14b,%ecx
  3e216c:	41 c1 e7 08          	shl    $0x8,%r15d
  3e2170:	41 0f b7 d7          	movzwl %r15w,%edx
  3e2174:	48 09 ca             	or     %rcx,%rdx
  3e2177:	41 0f b6 f4          	movzbl %r12b,%esi
  3e217b:	c1 e6 10             	shl    $0x10,%esi
  3e217e:	48 09 d6             	or     %rdx,%rsi
  3e2181:	48 09 c6             	or     %rax,%rsi
  3e2184:	48 89 df             	mov    %rbx,%rdi
  3e2187:	e8 84 d0 15 00       	call   53f210 <lua_pushinteger(lua_State*, long long)>
  3e218c:	b8 01 00 00 00       	mov    $0x1,%eax
  3e2191:	48 83 c4 08          	add    $0x8,%rsp
  3e2195:	5b                   	pop    %rbx
  3e2196:	41 5c                	pop    %r12
  3e2198:	41 5e                	pop    %r14
  3e219a:	41 5f                	pop    %r15
  3e219c:	c3                   	ret
  3e219d:	0f 1f 00             	nopl   (%rax)

00000000003e21a0 <app::script::(anonymous namespace)::PixelColor_rgbaR(lua_State*)>:
  3e21a0:	53                   	push   %rbx
  3e21a1:	48 89 fb             	mov    %rdi,%rbx
  3e21a4:	be 01 00 00 00       	mov    $0x1,%esi
  3e21a9:	31 d2                	xor    %edx,%edx
  3e21ab:	e8 00 c9 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3e21b0:	0f b6 f0             	movzbl %al,%esi
  3e21b3:	48 89 df             	mov    %rbx,%rdi
  3e21b6:	e8 55 d0 15 00       	call   53f210 <lua_pushinteger(lua_State*, long long)>
  3e21bb:	b8 01 00 00 00       	mov    $0x1,%eax
  3e21c0:	5b                   	pop    %rbx
  3e21c1:	c3                   	ret
  3e21c2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  3e21c9:	1f 84 00 00 00 00 00

00000000003e21d0 <app::script::(anonymous namespace)::PixelColor_rgbaG(lua_State*)>:
  3e21d0:	53                   	push   %rbx
  3e21d1:	48 89 fb             	mov    %rdi,%rbx
  3e21d4:	be 01 00 00 00       	mov    $0x1,%esi
  3e21d9:	31 d2                	xor    %edx,%edx
  3e21db:	e8 d0 c8 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3e21e0:	0f b6 f4             	movzbl %ah,%esi
  3e21e3:	48 89 df             	mov    %rbx,%rdi
  3e21e6:	e8 25 d0 15 00       	call   53f210 <lua_pushinteger(lua_State*, long long)>
  3e21eb:	b8 01 00 00 00       	mov    $0x1,%eax
  3e21f0:	5b                   	pop    %rbx
  3e21f1:	c3                   	ret
  3e21f2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  3e21f9:	1f 84 00 00 00 00 00

00000000003e2200 <app::script::(anonymous namespace)::PixelColor_rgbaB(lua_State*)>:
  3e2200:	53                   	push   %rbx
  3e2201:	48 89 fb             	mov    %rdi,%rbx
  3e2204:	be 01 00 00 00       	mov    $0x1,%esi
  3e2209:	31 d2                	xor    %edx,%edx
  3e220b:	e8 a0 c8 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3e2210:	c1 e8 10             	shr    $0x10,%eax
  3e2213:	0f b6 f0             	movzbl %al,%esi
  3e2216:	48 89 df             	mov    %rbx,%rdi
  3e2219:	e8 f2 cf 15 00       	call   53f210 <lua_pushinteger(lua_State*, long long)>
  3e221e:	b8 01 00 00 00       	mov    $0x1,%eax
  3e2223:	5b                   	pop    %rbx
  3e2224:	c3                   	ret
  3e2225:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  3e222c:	00 00 00 00

00000000003e2230 <app::script::(anonymous namespace)::PixelColor_rgbaA(lua_State*)>:
  3e2230:	53                   	push   %rbx
  3e2231:	48 89 fb             	mov    %rdi,%rbx
  3e2234:	be 01 00 00 00       	mov    $0x1,%esi
  3e2239:	31 d2                	xor    %edx,%edx
  3e223b:	e8 70 c8 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3e2240:	c1 e8 18             	shr    $0x18,%eax
  3e2243:	48 89 df             	mov    %rbx,%rdi
  3e2246:	48 89 c6             	mov    %rax,%rsi
  3e2249:	e8 c2 cf 15 00       	call   53f210 <lua_pushinteger(lua_State*, long long)>
  3e224e:	b8 01 00 00 00       	mov    $0x1,%eax
  3e2253:	5b                   	pop    %rbx
  3e2254:	c3                   	ret
