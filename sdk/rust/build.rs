/*
Copyright 2023 The Flame Authors.
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at
    http://www.apache.org/licenses/LICENSE-2.0
Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .type_attribute("flame.TaskState", "#[allow(clippy::enum_variant_names)]")
        .type_attribute("flame.Shim", "#[allow(clippy::enum_variant_names)]")
        .type_attribute(
            "flame.ExecutorState",
            "#[allow(clippy::enum_variant_names)]",
        )
        .protoc_arg("--experimental_allow_proto3_optional")
        .compile_protos(
            &[
                "protos/types.proto",
                "protos/frontend.proto",
                "protos/shim.proto",
            ],
            &["protos"],
        )?;

    Ok(())
}
