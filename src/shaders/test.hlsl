#define RSIGNATURE \
    "RootConstants(b0, num32BitConstants = 2), " \
    "DescriptorTable(SRV(t0, numDescriptors = 2)),"

struct Vertex {
    float3 position;
    float3 color;
};
StructuredBuffer<Vertex> srv_vertex_buffer : register(t0);
Buffer<uint> srv_index_buffer : register(t1);
struct DrawCallArgs {
    uint start_index_location;
    uint base_vertex_location;
};
ConstantBuffer<DrawCallArgs> cbv_drawcall : register(b0);

[RootSignature(RSIGNATURE)]
void main_vs(
    in uint vid : SV_VertexID,
    out float4 out_position : SV_Position,
    out float3 out_color : COLOR) {
    const uint vertex_index = srv_index_buffer[vid + cbv_drawcall.start_index_location] + cbv_drawcall.base_vertex_location;
    out_position = float4(srv_vertex_buffer[vertex_index].position, 1.0f);
    out_color = srv_vertex_buffer[vertex_index].color;
}

[RootSignature(RSIGNATURE)]
void main_ps(
    in float4 in_position : SV_Position,
    in float3 in_color : COLOR,
    out float4 out_color : SV_Target0) {
    out_color = float4(in_color, 1.0f);
}
