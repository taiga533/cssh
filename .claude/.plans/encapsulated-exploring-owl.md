# E2Eテストのマルチプラットフォーム対応

## 変更対象
- `.github/workflows/ci.yml`

## 変更内容

`e2e-test` ジョブに `matrix.os: [ubuntu-latest, windows-latest]` を追加し、各プラットフォームでE2Eテストを実行する。

### 具体的な変更点

1. **matrix戦略の追加**
   ```yaml
   strategy:
     matrix:
       os: [ubuntu-latest, windows-latest]
   runs-on: ${{ matrix.os }}
   ```

2. **ジョブ名の更新**
   ```yaml
   name: E2E Tests (${{ matrix.os }})
   ```

3. **Windows対応: bashシェル明示指定**
   - `run` ステップに `shell: bash` を指定（Windows runnerではデフォルトがpwshのため）

## 検証
- PRを作成してCIが両プラットフォームで実行されることを確認
