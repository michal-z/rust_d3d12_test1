#define RSIGNATURE "RootFlags(ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT)"

[RootSignature(RSIGNATURE)]
void main_vs(
    in float4 in_position : _Position,
    //in float3 in_normal : _Normal,
    out float4 out_position : SV_Position)
{
    out_position = float4(in_position.xyz, 1.0f);
}

[RootSignature(RSIGNATURE)]
void main_ps(
    in float4 in_position : SV_Position,
    out float4 out_color : SV_Target0)
{
    out_color = float4(0.0f, 0.6f, 0.0f, 1.0f);
}
