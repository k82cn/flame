// Code generated by protoc-gen-go. DO NOT EDIT.
// versions:
// 	protoc-gen-go v1.36.6
// 	protoc        v5.29.3
// source: backend.proto

package rpc

import (
	protoreflect "google.golang.org/protobuf/reflect/protoreflect"
	protoimpl "google.golang.org/protobuf/runtime/protoimpl"
	reflect "reflect"
	sync "sync"
	unsafe "unsafe"
)

const (
	// Verify that this generated code is sufficiently up-to-date.
	_ = protoimpl.EnforceVersion(20 - protoimpl.MinVersion)
	// Verify that runtime/protoimpl is sufficiently up-to-date.
	_ = protoimpl.EnforceVersion(protoimpl.MaxVersion - 20)
)

type RegisterExecutorRequest struct {
	state         protoimpl.MessageState `protogen:"open.v1"`
	ExecutorId    string                 `protobuf:"bytes,1,opt,name=executor_id,json=executorId,proto3" json:"executor_id,omitempty"`
	ExecutorSpec  *ExecutorSpec          `protobuf:"bytes,2,opt,name=executor_spec,json=executorSpec,proto3" json:"executor_spec,omitempty"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *RegisterExecutorRequest) Reset() {
	*x = RegisterExecutorRequest{}
	mi := &file_backend_proto_msgTypes[0]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *RegisterExecutorRequest) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*RegisterExecutorRequest) ProtoMessage() {}

func (x *RegisterExecutorRequest) ProtoReflect() protoreflect.Message {
	mi := &file_backend_proto_msgTypes[0]
	if x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use RegisterExecutorRequest.ProtoReflect.Descriptor instead.
func (*RegisterExecutorRequest) Descriptor() ([]byte, []int) {
	return file_backend_proto_rawDescGZIP(), []int{0}
}

func (x *RegisterExecutorRequest) GetExecutorId() string {
	if x != nil {
		return x.ExecutorId
	}
	return ""
}

func (x *RegisterExecutorRequest) GetExecutorSpec() *ExecutorSpec {
	if x != nil {
		return x.ExecutorSpec
	}
	return nil
}

type UnregisterExecutorRequest struct {
	state         protoimpl.MessageState `protogen:"open.v1"`
	ExecutorId    string                 `protobuf:"bytes,1,opt,name=executor_id,json=executorId,proto3" json:"executor_id,omitempty"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *UnregisterExecutorRequest) Reset() {
	*x = UnregisterExecutorRequest{}
	mi := &file_backend_proto_msgTypes[1]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *UnregisterExecutorRequest) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*UnregisterExecutorRequest) ProtoMessage() {}

func (x *UnregisterExecutorRequest) ProtoReflect() protoreflect.Message {
	mi := &file_backend_proto_msgTypes[1]
	if x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use UnregisterExecutorRequest.ProtoReflect.Descriptor instead.
func (*UnregisterExecutorRequest) Descriptor() ([]byte, []int) {
	return file_backend_proto_rawDescGZIP(), []int{1}
}

func (x *UnregisterExecutorRequest) GetExecutorId() string {
	if x != nil {
		return x.ExecutorId
	}
	return ""
}

type BindExecutorRequest struct {
	state         protoimpl.MessageState `protogen:"open.v1"`
	ExecutorId    string                 `protobuf:"bytes,1,opt,name=executor_id,json=executorId,proto3" json:"executor_id,omitempty"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *BindExecutorRequest) Reset() {
	*x = BindExecutorRequest{}
	mi := &file_backend_proto_msgTypes[2]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *BindExecutorRequest) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*BindExecutorRequest) ProtoMessage() {}

func (x *BindExecutorRequest) ProtoReflect() protoreflect.Message {
	mi := &file_backend_proto_msgTypes[2]
	if x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use BindExecutorRequest.ProtoReflect.Descriptor instead.
func (*BindExecutorRequest) Descriptor() ([]byte, []int) {
	return file_backend_proto_rawDescGZIP(), []int{2}
}

func (x *BindExecutorRequest) GetExecutorId() string {
	if x != nil {
		return x.ExecutorId
	}
	return ""
}

type BindExecutorResponse struct {
	state         protoimpl.MessageState `protogen:"open.v1"`
	Application   *Application           `protobuf:"bytes,1,opt,name=application,proto3" json:"application,omitempty"`
	Session       *Session               `protobuf:"bytes,2,opt,name=session,proto3" json:"session,omitempty"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *BindExecutorResponse) Reset() {
	*x = BindExecutorResponse{}
	mi := &file_backend_proto_msgTypes[3]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *BindExecutorResponse) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*BindExecutorResponse) ProtoMessage() {}

func (x *BindExecutorResponse) ProtoReflect() protoreflect.Message {
	mi := &file_backend_proto_msgTypes[3]
	if x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use BindExecutorResponse.ProtoReflect.Descriptor instead.
func (*BindExecutorResponse) Descriptor() ([]byte, []int) {
	return file_backend_proto_rawDescGZIP(), []int{3}
}

func (x *BindExecutorResponse) GetApplication() *Application {
	if x != nil {
		return x.Application
	}
	return nil
}

func (x *BindExecutorResponse) GetSession() *Session {
	if x != nil {
		return x.Session
	}
	return nil
}

type BindExecutorCompletedRequest struct {
	state         protoimpl.MessageState `protogen:"open.v1"`
	ExecutorId    string                 `protobuf:"bytes,1,opt,name=executor_id,json=executorId,proto3" json:"executor_id,omitempty"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *BindExecutorCompletedRequest) Reset() {
	*x = BindExecutorCompletedRequest{}
	mi := &file_backend_proto_msgTypes[4]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *BindExecutorCompletedRequest) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*BindExecutorCompletedRequest) ProtoMessage() {}

func (x *BindExecutorCompletedRequest) ProtoReflect() protoreflect.Message {
	mi := &file_backend_proto_msgTypes[4]
	if x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use BindExecutorCompletedRequest.ProtoReflect.Descriptor instead.
func (*BindExecutorCompletedRequest) Descriptor() ([]byte, []int) {
	return file_backend_proto_rawDescGZIP(), []int{4}
}

func (x *BindExecutorCompletedRequest) GetExecutorId() string {
	if x != nil {
		return x.ExecutorId
	}
	return ""
}

type UnbindExecutorRequest struct {
	state         protoimpl.MessageState `protogen:"open.v1"`
	ExecutorId    string                 `protobuf:"bytes,1,opt,name=executor_id,json=executorId,proto3" json:"executor_id,omitempty"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *UnbindExecutorRequest) Reset() {
	*x = UnbindExecutorRequest{}
	mi := &file_backend_proto_msgTypes[5]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *UnbindExecutorRequest) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*UnbindExecutorRequest) ProtoMessage() {}

func (x *UnbindExecutorRequest) ProtoReflect() protoreflect.Message {
	mi := &file_backend_proto_msgTypes[5]
	if x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use UnbindExecutorRequest.ProtoReflect.Descriptor instead.
func (*UnbindExecutorRequest) Descriptor() ([]byte, []int) {
	return file_backend_proto_rawDescGZIP(), []int{5}
}

func (x *UnbindExecutorRequest) GetExecutorId() string {
	if x != nil {
		return x.ExecutorId
	}
	return ""
}

type UnbindExecutorCompletedRequest struct {
	state         protoimpl.MessageState `protogen:"open.v1"`
	ExecutorId    string                 `protobuf:"bytes,1,opt,name=executor_id,json=executorId,proto3" json:"executor_id,omitempty"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *UnbindExecutorCompletedRequest) Reset() {
	*x = UnbindExecutorCompletedRequest{}
	mi := &file_backend_proto_msgTypes[6]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *UnbindExecutorCompletedRequest) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*UnbindExecutorCompletedRequest) ProtoMessage() {}

func (x *UnbindExecutorCompletedRequest) ProtoReflect() protoreflect.Message {
	mi := &file_backend_proto_msgTypes[6]
	if x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use UnbindExecutorCompletedRequest.ProtoReflect.Descriptor instead.
func (*UnbindExecutorCompletedRequest) Descriptor() ([]byte, []int) {
	return file_backend_proto_rawDescGZIP(), []int{6}
}

func (x *UnbindExecutorCompletedRequest) GetExecutorId() string {
	if x != nil {
		return x.ExecutorId
	}
	return ""
}

type LaunchTaskRequest struct {
	state         protoimpl.MessageState `protogen:"open.v1"`
	ExecutorId    string                 `protobuf:"bytes,1,opt,name=executor_id,json=executorId,proto3" json:"executor_id,omitempty"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *LaunchTaskRequest) Reset() {
	*x = LaunchTaskRequest{}
	mi := &file_backend_proto_msgTypes[7]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *LaunchTaskRequest) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*LaunchTaskRequest) ProtoMessage() {}

func (x *LaunchTaskRequest) ProtoReflect() protoreflect.Message {
	mi := &file_backend_proto_msgTypes[7]
	if x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use LaunchTaskRequest.ProtoReflect.Descriptor instead.
func (*LaunchTaskRequest) Descriptor() ([]byte, []int) {
	return file_backend_proto_rawDescGZIP(), []int{7}
}

func (x *LaunchTaskRequest) GetExecutorId() string {
	if x != nil {
		return x.ExecutorId
	}
	return ""
}

type LaunchTaskResponse struct {
	state protoimpl.MessageState `protogen:"open.v1"`
	// If no more task in the session, the result is empty.
	Task          *Task `protobuf:"bytes,1,opt,name=task,proto3,oneof" json:"task,omitempty"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *LaunchTaskResponse) Reset() {
	*x = LaunchTaskResponse{}
	mi := &file_backend_proto_msgTypes[8]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *LaunchTaskResponse) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*LaunchTaskResponse) ProtoMessage() {}

func (x *LaunchTaskResponse) ProtoReflect() protoreflect.Message {
	mi := &file_backend_proto_msgTypes[8]
	if x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use LaunchTaskResponse.ProtoReflect.Descriptor instead.
func (*LaunchTaskResponse) Descriptor() ([]byte, []int) {
	return file_backend_proto_rawDescGZIP(), []int{8}
}

func (x *LaunchTaskResponse) GetTask() *Task {
	if x != nil {
		return x.Task
	}
	return nil
}

type CompleteTaskRequest struct {
	state         protoimpl.MessageState `protogen:"open.v1"`
	ExecutorId    string                 `protobuf:"bytes,1,opt,name=executor_id,json=executorId,proto3" json:"executor_id,omitempty"`
	TaskOutput    []byte                 `protobuf:"bytes,2,opt,name=task_output,json=taskOutput,proto3,oneof" json:"task_output,omitempty"`
	unknownFields protoimpl.UnknownFields
	sizeCache     protoimpl.SizeCache
}

func (x *CompleteTaskRequest) Reset() {
	*x = CompleteTaskRequest{}
	mi := &file_backend_proto_msgTypes[9]
	ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
	ms.StoreMessageInfo(mi)
}

func (x *CompleteTaskRequest) String() string {
	return protoimpl.X.MessageStringOf(x)
}

func (*CompleteTaskRequest) ProtoMessage() {}

func (x *CompleteTaskRequest) ProtoReflect() protoreflect.Message {
	mi := &file_backend_proto_msgTypes[9]
	if x != nil {
		ms := protoimpl.X.MessageStateOf(protoimpl.Pointer(x))
		if ms.LoadMessageInfo() == nil {
			ms.StoreMessageInfo(mi)
		}
		return ms
	}
	return mi.MessageOf(x)
}

// Deprecated: Use CompleteTaskRequest.ProtoReflect.Descriptor instead.
func (*CompleteTaskRequest) Descriptor() ([]byte, []int) {
	return file_backend_proto_rawDescGZIP(), []int{9}
}

func (x *CompleteTaskRequest) GetExecutorId() string {
	if x != nil {
		return x.ExecutorId
	}
	return ""
}

func (x *CompleteTaskRequest) GetTaskOutput() []byte {
	if x != nil {
		return x.TaskOutput
	}
	return nil
}

var File_backend_proto protoreflect.FileDescriptor

const file_backend_proto_rawDesc = "" +
	"\n" +
	"\rbackend.proto\x12\x05flame\x1a\vtypes.proto\"t\n" +
	"\x17RegisterExecutorRequest\x12\x1f\n" +
	"\vexecutor_id\x18\x01 \x01(\tR\n" +
	"executorId\x128\n" +
	"\rexecutor_spec\x18\x02 \x01(\v2\x13.flame.ExecutorSpecR\fexecutorSpec\"<\n" +
	"\x19UnregisterExecutorRequest\x12\x1f\n" +
	"\vexecutor_id\x18\x01 \x01(\tR\n" +
	"executorId\"6\n" +
	"\x13BindExecutorRequest\x12\x1f\n" +
	"\vexecutor_id\x18\x01 \x01(\tR\n" +
	"executorId\"v\n" +
	"\x14BindExecutorResponse\x124\n" +
	"\vapplication\x18\x01 \x01(\v2\x12.flame.ApplicationR\vapplication\x12(\n" +
	"\asession\x18\x02 \x01(\v2\x0e.flame.SessionR\asession\"?\n" +
	"\x1cBindExecutorCompletedRequest\x12\x1f\n" +
	"\vexecutor_id\x18\x01 \x01(\tR\n" +
	"executorId\"8\n" +
	"\x15UnbindExecutorRequest\x12\x1f\n" +
	"\vexecutor_id\x18\x01 \x01(\tR\n" +
	"executorId\"A\n" +
	"\x1eUnbindExecutorCompletedRequest\x12\x1f\n" +
	"\vexecutor_id\x18\x01 \x01(\tR\n" +
	"executorId\"4\n" +
	"\x11LaunchTaskRequest\x12\x1f\n" +
	"\vexecutor_id\x18\x01 \x01(\tR\n" +
	"executorId\"C\n" +
	"\x12LaunchTaskResponse\x12$\n" +
	"\x04task\x18\x01 \x01(\v2\v.flame.TaskH\x00R\x04task\x88\x01\x01B\a\n" +
	"\x05_task\"l\n" +
	"\x13CompleteTaskRequest\x12\x1f\n" +
	"\vexecutor_id\x18\x01 \x01(\tR\n" +
	"executorId\x12$\n" +
	"\vtask_output\x18\x02 \x01(\fH\x00R\n" +
	"taskOutput\x88\x01\x01B\x0e\n" +
	"\f_task_output2\xc7\x04\n" +
	"\aBackend\x12C\n" +
	"\x10RegisterExecutor\x12\x1e.flame.RegisterExecutorRequest\x1a\r.flame.Result\"\x00\x12G\n" +
	"\x12UnregisterExecutor\x12 .flame.UnregisterExecutorRequest\x1a\r.flame.Result\"\x00\x12I\n" +
	"\fBindExecutor\x12\x1a.flame.BindExecutorRequest\x1a\x1b.flame.BindExecutorResponse\"\x00\x12M\n" +
	"\x15BindExecutorCompleted\x12#.flame.BindExecutorCompletedRequest\x1a\r.flame.Result\"\x00\x12?\n" +
	"\x0eUnbindExecutor\x12\x1c.flame.UnbindExecutorRequest\x1a\r.flame.Result\"\x00\x12Q\n" +
	"\x17UnbindExecutorCompleted\x12%.flame.UnbindExecutorCompletedRequest\x1a\r.flame.Result\"\x00\x12C\n" +
	"\n" +
	"LaunchTask\x12\x18.flame.LaunchTaskRequest\x1a\x19.flame.LaunchTaskResponse\"\x00\x12;\n" +
	"\fCompleteTask\x12\x1a.flame.CompleteTaskRequest\x1a\r.flame.Result\"\x00B&Z$github.com/flame-sh/flame/sdk/go/rpcb\x06proto3"

var (
	file_backend_proto_rawDescOnce sync.Once
	file_backend_proto_rawDescData []byte
)

func file_backend_proto_rawDescGZIP() []byte {
	file_backend_proto_rawDescOnce.Do(func() {
		file_backend_proto_rawDescData = protoimpl.X.CompressGZIP(unsafe.Slice(unsafe.StringData(file_backend_proto_rawDesc), len(file_backend_proto_rawDesc)))
	})
	return file_backend_proto_rawDescData
}

var file_backend_proto_msgTypes = make([]protoimpl.MessageInfo, 10)
var file_backend_proto_goTypes = []any{
	(*RegisterExecutorRequest)(nil),        // 0: flame.RegisterExecutorRequest
	(*UnregisterExecutorRequest)(nil),      // 1: flame.UnregisterExecutorRequest
	(*BindExecutorRequest)(nil),            // 2: flame.BindExecutorRequest
	(*BindExecutorResponse)(nil),           // 3: flame.BindExecutorResponse
	(*BindExecutorCompletedRequest)(nil),   // 4: flame.BindExecutorCompletedRequest
	(*UnbindExecutorRequest)(nil),          // 5: flame.UnbindExecutorRequest
	(*UnbindExecutorCompletedRequest)(nil), // 6: flame.UnbindExecutorCompletedRequest
	(*LaunchTaskRequest)(nil),              // 7: flame.LaunchTaskRequest
	(*LaunchTaskResponse)(nil),             // 8: flame.LaunchTaskResponse
	(*CompleteTaskRequest)(nil),            // 9: flame.CompleteTaskRequest
	(*ExecutorSpec)(nil),                   // 10: flame.ExecutorSpec
	(*Application)(nil),                    // 11: flame.Application
	(*Session)(nil),                        // 12: flame.Session
	(*Task)(nil),                           // 13: flame.Task
	(*Result)(nil),                         // 14: flame.Result
}
var file_backend_proto_depIdxs = []int32{
	10, // 0: flame.RegisterExecutorRequest.executor_spec:type_name -> flame.ExecutorSpec
	11, // 1: flame.BindExecutorResponse.application:type_name -> flame.Application
	12, // 2: flame.BindExecutorResponse.session:type_name -> flame.Session
	13, // 3: flame.LaunchTaskResponse.task:type_name -> flame.Task
	0,  // 4: flame.Backend.RegisterExecutor:input_type -> flame.RegisterExecutorRequest
	1,  // 5: flame.Backend.UnregisterExecutor:input_type -> flame.UnregisterExecutorRequest
	2,  // 6: flame.Backend.BindExecutor:input_type -> flame.BindExecutorRequest
	4,  // 7: flame.Backend.BindExecutorCompleted:input_type -> flame.BindExecutorCompletedRequest
	5,  // 8: flame.Backend.UnbindExecutor:input_type -> flame.UnbindExecutorRequest
	6,  // 9: flame.Backend.UnbindExecutorCompleted:input_type -> flame.UnbindExecutorCompletedRequest
	7,  // 10: flame.Backend.LaunchTask:input_type -> flame.LaunchTaskRequest
	9,  // 11: flame.Backend.CompleteTask:input_type -> flame.CompleteTaskRequest
	14, // 12: flame.Backend.RegisterExecutor:output_type -> flame.Result
	14, // 13: flame.Backend.UnregisterExecutor:output_type -> flame.Result
	3,  // 14: flame.Backend.BindExecutor:output_type -> flame.BindExecutorResponse
	14, // 15: flame.Backend.BindExecutorCompleted:output_type -> flame.Result
	14, // 16: flame.Backend.UnbindExecutor:output_type -> flame.Result
	14, // 17: flame.Backend.UnbindExecutorCompleted:output_type -> flame.Result
	8,  // 18: flame.Backend.LaunchTask:output_type -> flame.LaunchTaskResponse
	14, // 19: flame.Backend.CompleteTask:output_type -> flame.Result
	12, // [12:20] is the sub-list for method output_type
	4,  // [4:12] is the sub-list for method input_type
	4,  // [4:4] is the sub-list for extension type_name
	4,  // [4:4] is the sub-list for extension extendee
	0,  // [0:4] is the sub-list for field type_name
}

func init() { file_backend_proto_init() }
func file_backend_proto_init() {
	if File_backend_proto != nil {
		return
	}
	file_types_proto_init()
	file_backend_proto_msgTypes[8].OneofWrappers = []any{}
	file_backend_proto_msgTypes[9].OneofWrappers = []any{}
	type x struct{}
	out := protoimpl.TypeBuilder{
		File: protoimpl.DescBuilder{
			GoPackagePath: reflect.TypeOf(x{}).PkgPath(),
			RawDescriptor: unsafe.Slice(unsafe.StringData(file_backend_proto_rawDesc), len(file_backend_proto_rawDesc)),
			NumEnums:      0,
			NumMessages:   10,
			NumExtensions: 0,
			NumServices:   1,
		},
		GoTypes:           file_backend_proto_goTypes,
		DependencyIndexes: file_backend_proto_depIdxs,
		MessageInfos:      file_backend_proto_msgTypes,
	}.Build()
	File_backend_proto = out.File
	file_backend_proto_goTypes = nil
	file_backend_proto_depIdxs = nil
}
