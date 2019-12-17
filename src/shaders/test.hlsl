#define RSIGNATURE \
    "DescriptorTable(SRV(t0, numDescriptors = 1)),"

struct Vertex {
    float4 position;
};
StructuredBuffer<Vertex> srv_vertex_buffer : register(t0);

[RootSignature(RSIGNATURE)]
void main_vs(
    in uint vid : SV_VertexID,
    out float4 out_position : SV_Position) {
    out_position = float4(srv_vertex_buffer[vid].position.xyz, 1.0f);
}

[RootSignature(RSIGNATURE)]
void main_ps(
    in float4 in_position : SV_Position,
    out float4 out_color : SV_Target0) {
    out_color = float4(0.0f, 0.6f, 0.0f, 1.0f);
}
