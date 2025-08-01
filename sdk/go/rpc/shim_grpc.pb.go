// Code generated by protoc-gen-go-grpc. DO NOT EDIT.
// versions:
// - protoc-gen-go-grpc v1.5.1
// - protoc             v5.29.3
// source: shim.proto

package rpc

import (
	context "context"
	grpc "google.golang.org/grpc"
	codes "google.golang.org/grpc/codes"
	status "google.golang.org/grpc/status"
)

// This is a compile-time assertion to ensure that this generated file
// is compatible with the grpc package it is being compiled against.
// Requires gRPC-Go v1.64.0 or later.
const _ = grpc.SupportPackageIsVersion9

const (
	GrpcShim_OnSessionEnter_FullMethodName = "/flame.GrpcShim/OnSessionEnter"
	GrpcShim_OnTaskInvoke_FullMethodName   = "/flame.GrpcShim/OnTaskInvoke"
	GrpcShim_OnSessionLeave_FullMethodName = "/flame.GrpcShim/OnSessionLeave"
)

// GrpcShimClient is the client API for GrpcShim service.
//
// For semantics around ctx use and closing/ending streaming RPCs, please refer to https://pkg.go.dev/google.golang.org/grpc/?tab=doc#ClientConn.NewStream.
type GrpcShimClient interface {
	OnSessionEnter(ctx context.Context, in *SessionContext, opts ...grpc.CallOption) (*Result, error)
	OnTaskInvoke(ctx context.Context, in *TaskContext, opts ...grpc.CallOption) (*TaskOutput, error)
	OnSessionLeave(ctx context.Context, in *EmptyRequest, opts ...grpc.CallOption) (*Result, error)
}

type grpcShimClient struct {
	cc grpc.ClientConnInterface
}

func NewGrpcShimClient(cc grpc.ClientConnInterface) GrpcShimClient {
	return &grpcShimClient{cc}
}

func (c *grpcShimClient) OnSessionEnter(ctx context.Context, in *SessionContext, opts ...grpc.CallOption) (*Result, error) {
	cOpts := append([]grpc.CallOption{grpc.StaticMethod()}, opts...)
	out := new(Result)
	err := c.cc.Invoke(ctx, GrpcShim_OnSessionEnter_FullMethodName, in, out, cOpts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *grpcShimClient) OnTaskInvoke(ctx context.Context, in *TaskContext, opts ...grpc.CallOption) (*TaskOutput, error) {
	cOpts := append([]grpc.CallOption{grpc.StaticMethod()}, opts...)
	out := new(TaskOutput)
	err := c.cc.Invoke(ctx, GrpcShim_OnTaskInvoke_FullMethodName, in, out, cOpts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *grpcShimClient) OnSessionLeave(ctx context.Context, in *EmptyRequest, opts ...grpc.CallOption) (*Result, error) {
	cOpts := append([]grpc.CallOption{grpc.StaticMethod()}, opts...)
	out := new(Result)
	err := c.cc.Invoke(ctx, GrpcShim_OnSessionLeave_FullMethodName, in, out, cOpts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

// GrpcShimServer is the server API for GrpcShim service.
// All implementations must embed UnimplementedGrpcShimServer
// for forward compatibility.
type GrpcShimServer interface {
	OnSessionEnter(context.Context, *SessionContext) (*Result, error)
	OnTaskInvoke(context.Context, *TaskContext) (*TaskOutput, error)
	OnSessionLeave(context.Context, *EmptyRequest) (*Result, error)
	mustEmbedUnimplementedGrpcShimServer()
}

// UnimplementedGrpcShimServer must be embedded to have
// forward compatible implementations.
//
// NOTE: this should be embedded by value instead of pointer to avoid a nil
// pointer dereference when methods are called.
type UnimplementedGrpcShimServer struct{}

func (UnimplementedGrpcShimServer) OnSessionEnter(context.Context, *SessionContext) (*Result, error) {
	return nil, status.Errorf(codes.Unimplemented, "method OnSessionEnter not implemented")
}
func (UnimplementedGrpcShimServer) OnTaskInvoke(context.Context, *TaskContext) (*TaskOutput, error) {
	return nil, status.Errorf(codes.Unimplemented, "method OnTaskInvoke not implemented")
}
func (UnimplementedGrpcShimServer) OnSessionLeave(context.Context, *EmptyRequest) (*Result, error) {
	return nil, status.Errorf(codes.Unimplemented, "method OnSessionLeave not implemented")
}
func (UnimplementedGrpcShimServer) mustEmbedUnimplementedGrpcShimServer() {}
func (UnimplementedGrpcShimServer) testEmbeddedByValue()                  {}

// UnsafeGrpcShimServer may be embedded to opt out of forward compatibility for this service.
// Use of this interface is not recommended, as added methods to GrpcShimServer will
// result in compilation errors.
type UnsafeGrpcShimServer interface {
	mustEmbedUnimplementedGrpcShimServer()
}

func RegisterGrpcShimServer(s grpc.ServiceRegistrar, srv GrpcShimServer) {
	// If the following call pancis, it indicates UnimplementedGrpcShimServer was
	// embedded by pointer and is nil.  This will cause panics if an
	// unimplemented method is ever invoked, so we test this at initialization
	// time to prevent it from happening at runtime later due to I/O.
	if t, ok := srv.(interface{ testEmbeddedByValue() }); ok {
		t.testEmbeddedByValue()
	}
	s.RegisterService(&GrpcShim_ServiceDesc, srv)
}

func _GrpcShim_OnSessionEnter_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(SessionContext)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(GrpcShimServer).OnSessionEnter(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: GrpcShim_OnSessionEnter_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(GrpcShimServer).OnSessionEnter(ctx, req.(*SessionContext))
	}
	return interceptor(ctx, in, info, handler)
}

func _GrpcShim_OnTaskInvoke_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(TaskContext)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(GrpcShimServer).OnTaskInvoke(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: GrpcShim_OnTaskInvoke_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(GrpcShimServer).OnTaskInvoke(ctx, req.(*TaskContext))
	}
	return interceptor(ctx, in, info, handler)
}

func _GrpcShim_OnSessionLeave_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(EmptyRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(GrpcShimServer).OnSessionLeave(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: GrpcShim_OnSessionLeave_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(GrpcShimServer).OnSessionLeave(ctx, req.(*EmptyRequest))
	}
	return interceptor(ctx, in, info, handler)
}

// GrpcShim_ServiceDesc is the grpc.ServiceDesc for GrpcShim service.
// It's only intended for direct use with grpc.RegisterService,
// and not to be introspected or modified (even as a copy)
var GrpcShim_ServiceDesc = grpc.ServiceDesc{
	ServiceName: "flame.GrpcShim",
	HandlerType: (*GrpcShimServer)(nil),
	Methods: []grpc.MethodDesc{
		{
			MethodName: "OnSessionEnter",
			Handler:    _GrpcShim_OnSessionEnter_Handler,
		},
		{
			MethodName: "OnTaskInvoke",
			Handler:    _GrpcShim_OnTaskInvoke_Handler,
		},
		{
			MethodName: "OnSessionLeave",
			Handler:    _GrpcShim_OnSessionLeave_Handler,
		},
	},
	Streams:  []grpc.StreamDesc{},
	Metadata: "shim.proto",
}

const (
	GrpcServiceManager_RegisterService_FullMethodName = "/flame.GrpcServiceManager/RegisterService"
)

// GrpcServiceManagerClient is the client API for GrpcServiceManager service.
//
// For semantics around ctx use and closing/ending streaming RPCs, please refer to https://pkg.go.dev/google.golang.org/grpc/?tab=doc#ClientConn.NewStream.
type GrpcServiceManagerClient interface {
	RegisterService(ctx context.Context, in *RegisterServiceRequest, opts ...grpc.CallOption) (*RegisterServiceResponse, error)
}

type grpcServiceManagerClient struct {
	cc grpc.ClientConnInterface
}

func NewGrpcServiceManagerClient(cc grpc.ClientConnInterface) GrpcServiceManagerClient {
	return &grpcServiceManagerClient{cc}
}

func (c *grpcServiceManagerClient) RegisterService(ctx context.Context, in *RegisterServiceRequest, opts ...grpc.CallOption) (*RegisterServiceResponse, error) {
	cOpts := append([]grpc.CallOption{grpc.StaticMethod()}, opts...)
	out := new(RegisterServiceResponse)
	err := c.cc.Invoke(ctx, GrpcServiceManager_RegisterService_FullMethodName, in, out, cOpts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

// GrpcServiceManagerServer is the server API for GrpcServiceManager service.
// All implementations must embed UnimplementedGrpcServiceManagerServer
// for forward compatibility.
type GrpcServiceManagerServer interface {
	RegisterService(context.Context, *RegisterServiceRequest) (*RegisterServiceResponse, error)
	mustEmbedUnimplementedGrpcServiceManagerServer()
}

// UnimplementedGrpcServiceManagerServer must be embedded to have
// forward compatible implementations.
//
// NOTE: this should be embedded by value instead of pointer to avoid a nil
// pointer dereference when methods are called.
type UnimplementedGrpcServiceManagerServer struct{}

func (UnimplementedGrpcServiceManagerServer) RegisterService(context.Context, *RegisterServiceRequest) (*RegisterServiceResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method RegisterService not implemented")
}
func (UnimplementedGrpcServiceManagerServer) mustEmbedUnimplementedGrpcServiceManagerServer() {}
func (UnimplementedGrpcServiceManagerServer) testEmbeddedByValue()                            {}

// UnsafeGrpcServiceManagerServer may be embedded to opt out of forward compatibility for this service.
// Use of this interface is not recommended, as added methods to GrpcServiceManagerServer will
// result in compilation errors.
type UnsafeGrpcServiceManagerServer interface {
	mustEmbedUnimplementedGrpcServiceManagerServer()
}

func RegisterGrpcServiceManagerServer(s grpc.ServiceRegistrar, srv GrpcServiceManagerServer) {
	// If the following call pancis, it indicates UnimplementedGrpcServiceManagerServer was
	// embedded by pointer and is nil.  This will cause panics if an
	// unimplemented method is ever invoked, so we test this at initialization
	// time to prevent it from happening at runtime later due to I/O.
	if t, ok := srv.(interface{ testEmbeddedByValue() }); ok {
		t.testEmbeddedByValue()
	}
	s.RegisterService(&GrpcServiceManager_ServiceDesc, srv)
}

func _GrpcServiceManager_RegisterService_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(RegisterServiceRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(GrpcServiceManagerServer).RegisterService(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: GrpcServiceManager_RegisterService_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(GrpcServiceManagerServer).RegisterService(ctx, req.(*RegisterServiceRequest))
	}
	return interceptor(ctx, in, info, handler)
}

// GrpcServiceManager_ServiceDesc is the grpc.ServiceDesc for GrpcServiceManager service.
// It's only intended for direct use with grpc.RegisterService,
// and not to be introspected or modified (even as a copy)
var GrpcServiceManager_ServiceDesc = grpc.ServiceDesc{
	ServiceName: "flame.GrpcServiceManager",
	HandlerType: (*GrpcServiceManagerServer)(nil),
	Methods: []grpc.MethodDesc{
		{
			MethodName: "RegisterService",
			Handler:    _GrpcServiceManager_RegisterService_Handler,
		},
	},
	Streams:  []grpc.StreamDesc{},
	Metadata: "shim.proto",
}
