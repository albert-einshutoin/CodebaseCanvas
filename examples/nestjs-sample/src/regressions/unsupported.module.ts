import { Module, Injectable, Inject, forwardRef } from '@nestjs/common';
import type { DynamicModule } from '@nestjs/common';
import { UsersModule } from '../users/users.module';
import { UsersService } from '../users/users.service';
import { Port } from './ports';
import type { TypeOnlyToken } from './ports';

@Injectable()
export class UnsupportedConsumer {
  constructor(
    private readonly port: Port,
    private readonly typed: TypeOnlyToken,
    @Inject('TOKEN') private readonly custom: string,
  ) {}
}
@Module({})
export class DynamicFeature {
  static register(): DynamicModule { return { module: DynamicFeature }; }
}
const sharedProviders = [UsersService];
@Module({
  imports: [forwardRef(() => UsersModule), DynamicFeature.register()],
  providers: [
    UnsupportedConsumer,
    ...sharedProviders,
    { provide: 'FACTORY', useFactory: () => 'factory-value' },
  ],
})
export class UnsupportedModule {}
