#define RSIGNATURE \
    "RootConstants(b0, num32BitConstants = 3), " \
    "DescriptorTable(SRV(t0, numDescriptors = 3)),"

struct Vertex {
    float3 position;
    float3 color;
};

struct Constants0 {
    uint start_index_location;
    uint base_vertex_location;
    uint transform_location;
};

struct Transform {
    float4x4 clip_from_object;
};

ConstantBuffer<Constants0> cbv_0 : register(b0);
StructuredBuffer<Vertex> srv_vertex_buffer : register(t0);
Buffer<uint> srv_index_buffer : register(t1);
StructuredBuffer<Transform> srv_transforms : register(t2);

[RootSignature(RSIGNATURE)]
void main_vs(
    in uint vid : SV_VertexID,
    out float4 out_position : SV_Position,
    out float3 out_color : COLOR) {

    uint vertex_index = srv_index_buffer[vid + cbv_0.start_index_location] + cbv_0.base_vertex_location;
    Vertex vertex = srv_vertex_buffer[vertex_index];
    Transform t = srv_transforms[cbv_0.transform_location];

    out_position = mul(t.clip_from_object, float4(vertex.position, 1.0f));
    out_color = vertex.color;
}

[RootSignature(RSIGNATURE)]
void main_ps(
    in float4 in_position : SV_Position,
    in float3 in_color : COLOR,
    out float4 out_color : SV_Target0) {

    out_color = float4(in_color, 1.0f);
}
