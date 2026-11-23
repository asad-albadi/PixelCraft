#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import tempfile
import json
import unittest
import os
import shutil
import subprocess
import zipfile

ROOT=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location('android_release',ROOT/'checks/build-android-release.py')
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class SigningEnvironmentTests(unittest.TestCase):
    def test_missing_credentials_fail_closed(self):
        with self.assertRaises(ValueError): module.signing_environment(None,{})

    def test_private_config_resolves_relative_key_without_exposing_passwords(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);key=root/'release.jks';key.write_bytes(b'fixture');key.chmod(0o600)
            config=root/'credentials.json'
            config.write_text(json.dumps({'keystore':'release.jks','store_password':'fixture-store','key_password':'fixture-key','key_alias':'fixture-alias'}));config.chmod(0o600)
            env=module.signing_environment(config,{})
            self.assertEqual(env['PIXELCRAFT_ANDROID_KEYSTORE'],str(key))
            self.assertEqual(env['PIXELCRAFT_ANDROID_KEY_ALIAS'],'fixture-alias')
            self.assertEqual(env['PIXELCRAFT_ANDROID_STORE_PASSWORD'],'fixture-store')

    def test_readable_credentials_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            config=Path(tmp)/'credentials.json';config.write_text('{}');config.chmod(0o644)
            with self.assertRaises(ValueError): module.signing_environment(config,{})

    def test_environment_requires_real_private_key_file(self):
        values={name:'fixture' for name in module.SIGNING_VARIABLES}
        values['PIXELCRAFT_ANDROID_KEYSTORE']='/nonexistent/pixelcraft-release.jks'
        with self.assertRaises(ValueError): module.signing_environment(None,values)

    def test_release_version_cannot_escape_output_directory(self):
        with self.assertRaises(ValueError):module.artifact_prefix('../../bad')
        self.assertEqual(module.artifact_prefix('v0.12.0'),'PixelCraft-v0.12.0-Android')

    def test_tag_must_match_pubspec_and_build_code_is_positive(self):
        self.assertEqual(module.release_identity('0.12.0+2', 'v0.12.0'), ('v0.12.0', '0.12.0', 2))
        with self.assertRaises(ValueError):module.release_identity('0.12.0+2', 'v0.13.0')
        with self.assertRaises(ValueError):module.release_identity('0.12.0+0', 'v0.12.0')

    def test_new_android_release_must_increment_version_code(self):
        module.check_version_progression('0.13.0', 3, '0.12.0+2')
        module.check_version_progression('0.12.0', 2, None)
        with self.assertRaises(ValueError):module.check_version_progression('0.13.0', 2, '0.12.0+2')
        with self.assertRaises(ValueError):module.check_version_progression('0.12.0', 1, '0.12.0+2')

    @unittest.skipUnless(shutil.which('keytool') and shutil.which('jarsigner'), 'Java signing tools needed')
    def test_bundle_rejects_appended_unsigned_payload(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);key=root/'fixture.jks';bundle=root/'fixture.aab'
            env=dict(os.environ, FIXTURE_PASSWORD='temporary-test-password')
            subprocess.run(['keytool','-genkeypair','-keystore',str(key),'-alias','fixture','-keyalg','RSA','-validity','1','-dname','CN=Test fixture','-storepass:env','FIXTURE_PASSWORD','-keypass:env','FIXTURE_PASSWORD','-noprompt'],env=env,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            with zipfile.ZipFile(bundle,'w') as archive:archive.writestr('base/payload.txt','signed')
            subprocess.run(['jarsigner','-keystore',str(key),'-storepass:env','FIXTURE_PASSWORD',str(bundle),'fixture'],env=env,check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            module.verify_bundle_signature(bundle, env)
            with zipfile.ZipFile(bundle,'a') as archive:archive.writestr('base/unsigned.txt','unexpected')
            with self.assertRaises(ValueError):module.verify_bundle_signature(bundle,env)

if __name__=='__main__':unittest.main()
