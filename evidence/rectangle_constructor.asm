
aseprite:     file format elf64-x86-64


Disassembly of section .text:

00000000003e9ea0 <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)>:
  3e9ea0:	55                   	push   %rbp
  3e9ea1:	41 57                	push   %r15
  3e9ea3:	41 56                	push   %r14
  3e9ea5:	41 55                	push   %r13
  3e9ea7:	41 54                	push   %r12
  3e9ea9:	53                   	push   %rbx
  3e9eaa:	50                   	push   %rax
  3e9eab:	41 89 d7             	mov    %edx,%r15d
  3e9eae:	49 89 f6             	mov    %rsi,%r14
  3e9eb1:	48 89 fb             	mov    %rdi,%rbx
  3e9eb4:	48 8d 15 45 6d b5 00 	lea    0xb56d45(%rip),%rdx        # f40c00 <gUnaryActiveEdge+0xd49c>
  3e9ebb:	48 89 f7             	mov    %rsi,%rdi
  3e9ebe:	44 89 fe             	mov    %r15d,%esi
  3e9ec1:	e8 3a 54 14 00       	call   52f300 <luaL_testudata(lua_State*, int, char const*)>
  3e9ec6:	48 85 c0             	test   %rax,%rax
  3e9ec9:	74 15                	je     3e9ee0 <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)+0x40>
  3e9ecb:	0f 10 00             	movups (%rax),%xmm0
  3e9ece:	0f 11 03             	movups %xmm0,(%rbx)
  3e9ed1:	48 83 c4 08          	add    $0x8,%rsp
  3e9ed5:	5b                   	pop    %rbx
  3e9ed6:	41 5c                	pop    %r12
  3e9ed8:	41 5d                	pop    %r13
  3e9eda:	41 5e                	pop    %r14
  3e9edc:	41 5f                	pop    %r15
  3e9ede:	5d                   	pop    %rbp
  3e9edf:	c3                   	ret
  3e9ee0:	4c 89 f7             	mov    %r14,%rdi
  3e9ee3:	44 89 fe             	mov    %r15d,%esi
  3e9ee6:	e8 85 43 15 00       	call   53e270 <lua_type(lua_State*, int)>
  3e9eeb:	83 f8 05             	cmp    $0x5,%eax
  3e9eee:	75 58                	jne    3e9f48 <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)+0xa8>
  3e9ef0:	0f 57 c0             	xorps  %xmm0,%xmm0
  3e9ef3:	0f 11 03             	movups %xmm0,(%rbx)
  3e9ef6:	48 8d 15 e4 8e b7 00 	lea    0xb78ee4(%rip),%rdx        # f62de1 <doProlog.enumValueStart+0xa1b>
  3e9efd:	4c 89 f7             	mov    %r14,%rdi
  3e9f00:	44 89 fe             	mov    %r15d,%esi
  3e9f03:	e8 08 58 15 00       	call   53f710 <lua_getfield(lua_State*, int, char const*)>
  3e9f08:	85 c0                	test   %eax,%eax
  3e9f0a:	0f 8e ec 00 00 00    	jle    3e9ffc <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)+0x15c>
  3e9f10:	48 8d 15 ac e6 b6 00 	lea    0xb6e6ac(%rip),%rdx        # f585c3 <KW_UTF_8+0xd7aa>
  3e9f17:	4c 89 f7             	mov    %r14,%rdi
  3e9f1a:	44 89 fe             	mov    %r15d,%esi
  3e9f1d:	e8 ee 57 15 00       	call   53f710 <lua_getfield(lua_State*, int, char const*)>
  3e9f22:	48 8d 15 fd 6c b7 00 	lea    0xb76cfd(%rip),%rdx        # f60c26 <KW_UTF_8+0x15e0d>
  3e9f29:	4c 89 f7             	mov    %r14,%rdi
  3e9f2c:	44 89 fe             	mov    %r15d,%esi
  3e9f2f:	e8 dc 57 15 00       	call   53f710 <lua_getfield(lua_State*, int, char const*)>
  3e9f34:	85 c0                	test   %eax,%eax
  3e9f36:	0f 8e 17 01 00 00    	jle    3ea053 <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)+0x1b3>
  3e9f3c:	48 8d 15 f6 6c b7 00 	lea    0xb76cf6(%rip),%rdx        # f60c39 <KW_UTF_8+0x15e20>
  3e9f43:	e9 31 01 00 00       	jmp    3ea079 <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)+0x1d9>
  3e9f48:	45 85 ff             	test   %r15d,%r15d
  3e9f4b:	0f 8e fa 00 00 00    	jle    3ea04b <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)+0x1ab>
  3e9f51:	4c 89 f7             	mov    %r14,%rdi
  3e9f54:	e8 e7 3e 15 00       	call   53de40 <lua_gettop(lua_State*)>
  3e9f59:	41 8d 6f 01          	lea    0x1(%r15),%ebp
  3e9f5d:	44 39 f8             	cmp    %r15d,%eax
  3e9f60:	7e 47                	jle    3e9fa9 <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)+0x109>
  3e9f62:	e8 19 a8 ff ff       	call   3e4780 <char const* app::script::get_mtname<gfx::PointT<int> >()>
  3e9f67:	4c 89 f7             	mov    %r14,%rdi
  3e9f6a:	44 89 fe             	mov    %r15d,%esi
  3e9f6d:	48 89 c2             	mov    %rax,%rdx
  3e9f70:	e8 8b 53 14 00       	call   52f300 <luaL_testudata(lua_State*, int, char const*)>
  3e9f75:	49 89 c4             	mov    %rax,%r12
  3e9f78:	e8 43 52 00 00       	call   3ef1c0 <char const* app::script::get_mtname<gfx::SizeT<int> >()>
  3e9f7d:	4c 89 f7             	mov    %r14,%rdi
  3e9f80:	89 ee                	mov    %ebp,%esi
  3e9f82:	48 89 c2             	mov    %rax,%rdx
  3e9f85:	e8 76 53 14 00       	call   52f300 <luaL_testudata(lua_State*, int, char const*)>
  3e9f8a:	4d 85 e4             	test   %r12,%r12
  3e9f8d:	74 1a                	je     3e9fa9 <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)+0x109>
  3e9f8f:	48 85 c0             	test   %rax,%rax
  3e9f92:	74 15                	je     3e9fa9 <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)+0x109>
  3e9f94:	f2 0f 10 00          	movsd  (%rax),%xmm0
  3e9f98:	f2 41 0f 10 0c 24    	movsd  (%r12),%xmm1
  3e9f9e:	0f 16 c8             	movlhps %xmm0,%xmm1
  3e9fa1:	0f 11 0b             	movups %xmm1,(%rbx)
  3e9fa4:	e9 28 ff ff ff       	jmp    3e9ed1 <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)+0x31>
  3e9fa9:	4c 89 f7             	mov    %r14,%rdi
  3e9fac:	44 89 fe             	mov    %r15d,%esi
  3e9faf:	31 d2                	xor    %edx,%edx
  3e9fb1:	e8 fa 4a 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3e9fb6:	49 89 c4             	mov    %rax,%r12
  3e9fb9:	4c 89 f7             	mov    %r14,%rdi
  3e9fbc:	89 ee                	mov    %ebp,%esi
  3e9fbe:	31 d2                	xor    %edx,%edx
  3e9fc0:	e8 eb 4a 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3e9fc5:	49 89 c5             	mov    %rax,%r13
  3e9fc8:	41 8d 77 02          	lea    0x2(%r15),%esi
  3e9fcc:	4c 89 f7             	mov    %r14,%rdi
  3e9fcf:	31 d2                	xor    %edx,%edx
  3e9fd1:	e8 da 4a 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3e9fd6:	48 89 c5             	mov    %rax,%rbp
  3e9fd9:	41 83 c7 03          	add    $0x3,%r15d
  3e9fdd:	4c 89 f7             	mov    %r14,%rdi
  3e9fe0:	44 89 fe             	mov    %r15d,%esi
  3e9fe3:	31 d2                	xor    %edx,%edx
  3e9fe5:	e8 c6 4a 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3e9fea:	44 89 23             	mov    %r12d,(%rbx)
  3e9fed:	44 89 6b 04          	mov    %r13d,0x4(%rbx)
  3e9ff1:	89 6b 08             	mov    %ebp,0x8(%rbx)
  3e9ff4:	89 43 0c             	mov    %eax,0xc(%rbx)
  3e9ff7:	e9 d5 fe ff ff       	jmp    3e9ed1 <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)+0x31>
  3e9ffc:	4c 89 f7             	mov    %r14,%rdi
  3e9fff:	be fe ff ff ff       	mov    $0xfffffffe,%esi
  3ea004:	e8 57 3e 15 00       	call   53de60 <lua_settop(lua_State*, int)>
  3ea009:	ba 01 00 00 00       	mov    $0x1,%edx
  3ea00e:	4c 89 f7             	mov    %r14,%rdi
  3ea011:	44 89 fe             	mov    %r15d,%esi
  3ea014:	e8 17 58 15 00       	call   53f830 <lua_geti(lua_State*, int, long long)>
  3ea019:	ba 02 00 00 00       	mov    $0x2,%edx
  3ea01e:	4c 89 f7             	mov    %r14,%rdi
  3ea021:	44 89 fe             	mov    %r15d,%esi
  3ea024:	e8 07 58 15 00       	call   53f830 <lua_geti(lua_State*, int, long long)>
  3ea029:	ba 03 00 00 00       	mov    $0x3,%edx
  3ea02e:	4c 89 f7             	mov    %r14,%rdi
  3ea031:	44 89 fe             	mov    %r15d,%esi
  3ea034:	e8 f7 57 15 00       	call   53f830 <lua_geti(lua_State*, int, long long)>
  3ea039:	ba 04 00 00 00       	mov    $0x4,%edx
  3ea03e:	4c 89 f7             	mov    %r14,%rdi
  3ea041:	44 89 fe             	mov    %r15d,%esi
  3ea044:	e8 e7 57 15 00       	call   53f830 <lua_geti(lua_State*, int, long long)>
  3ea049:	eb 39                	jmp    3ea084 <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)+0x1e4>
  3ea04b:	0f 57 c0             	xorps  %xmm0,%xmm0
  3ea04e:	e9 7b fe ff ff       	jmp    3e9ece <app::script::(anonymous namespace)::Rectangle_new(lua_State*, int)+0x2e>
  3ea053:	4c 89 f7             	mov    %r14,%rdi
  3ea056:	be fe ff ff ff       	mov    $0xfffffffe,%esi
  3ea05b:	e8 00 3e 15 00       	call   53de60 <lua_settop(lua_State*, int)>
  3ea060:	48 8d 15 a2 e5 b6 00 	lea    0xb6e5a2(%rip),%rdx        # f58609 <KW_UTF_8+0xd7f0>
  3ea067:	4c 89 f7             	mov    %r14,%rdi
  3ea06a:	44 89 fe             	mov    %r15d,%esi
  3ea06d:	e8 9e 56 15 00       	call   53f710 <lua_getfield(lua_State*, int, char const*)>
  3ea072:	48 8d 15 55 81 b7 00 	lea    0xb78155(%rip),%rdx        # f621ce <KW_UTF_8+0x173b5>
  3ea079:	4c 89 f7             	mov    %r14,%rdi
  3ea07c:	44 89 fe             	mov    %r15d,%esi
  3ea07f:	e8 8c 56 15 00       	call   53f710 <lua_getfield(lua_State*, int, char const*)>
  3ea084:	4c 89 f7             	mov    %r14,%rdi
  3ea087:	be fc ff ff ff       	mov    $0xfffffffc,%esi
  3ea08c:	31 d2                	xor    %edx,%edx
  3ea08e:	e8 1d 4a 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3ea093:	89 03                	mov    %eax,(%rbx)
  3ea095:	4c 89 f7             	mov    %r14,%rdi
  3ea098:	be fd ff ff ff       	mov    $0xfffffffd,%esi
  3ea09d:	31 d2                	xor    %edx,%edx
  3ea09f:	e8 0c 4a 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3ea0a4:	89 43 04             	mov    %eax,0x4(%rbx)
  3ea0a7:	4c 89 f7             	mov    %r14,%rdi
  3ea0aa:	be fe ff ff ff       	mov    $0xfffffffe,%esi
  3ea0af:	31 d2                	xor    %edx,%edx
  3ea0b1:	e8 fa 49 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3ea0b6:	89 43 08             	mov    %eax,0x8(%rbx)
  3ea0b9:	4c 89 f7             	mov    %r14,%rdi
  3ea0bc:	be ff ff ff ff       	mov    $0xffffffff,%esi
  3ea0c1:	31 d2                	xor    %edx,%edx
  3ea0c3:	e8 e8 49 15 00       	call   53eab0 <lua_tointegerx(lua_State*, int, int*)>
  3ea0c8:	89 43 0c             	mov    %eax,0xc(%rbx)
  3ea0cb:	4c 89 f7             	mov    %r14,%rdi
  3ea0ce:	be fb ff ff ff       	mov    $0xfffffffb,%esi
  3ea0d3:	48 83 c4 08          	add    $0x8,%rsp
  3ea0d7:	5b                   	pop    %rbx
  3ea0d8:	41 5c                	pop    %r12
  3ea0da:	41 5d                	pop    %r13
  3ea0dc:	41 5e                	pop    %r14
  3ea0de:	41 5f                	pop    %r15
  3ea0e0:	5d                   	pop    %rbp
  3ea0e1:	e9 7a 3d 15 00       	jmp    53de60 <lua_settop(lua_State*, int)>
  3ea0e6:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  3ea0ed:	00 00 00

00000000003ea0f0 <app::script::(anonymous namespace)::Rectangle_gc(lua_State*)>:
  3ea0f0:	50                   	push   %rax
  3ea0f1:	48 8d 15 08 6b b5 00 	lea    0xb56b08(%rip),%rdx        # f40c00 <gUnaryActiveEdge+0xd49c>
  3ea0f8:	be 01 00 00 00       	mov    $0x1,%esi
  3ea0fd:	e8 7e 52 14 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3ea102:	31 c0                	xor    %eax,%eax
  3ea104:	59                   	pop    %rcx
  3ea105:	c3                   	ret
  3ea106:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  3ea10d:	00 00 00

00000000003ea110 <app::script::(anonymous namespace)::Rectangle_eq(lua_State*)>:
  3ea110:	41 57                	push   %r15
  3ea112:	41 56                	push   %r14
  3ea114:	53                   	push   %rbx
  3ea115:	48 89 fb             	mov    %rdi,%rbx
  3ea118:	4c 8d 3d e1 6a b5 00 	lea    0xb56ae1(%rip),%r15        # f40c00 <gUnaryActiveEdge+0xd49c>
  3ea11f:	be 01 00 00 00       	mov    $0x1,%esi
  3ea124:	4c 89 fa             	mov    %r15,%rdx
  3ea127:	e8 54 52 14 00       	call   52f380 <luaL_checkudata(lua_State*, int, char const*)>
  3ea12c:	49 89 c6             	mov    %rax,%r14
  3ea12f:	48                   	rex.W
